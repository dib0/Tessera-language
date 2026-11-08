// TesseraModule - native IIS 7+ module that runs a Tessera web application
// built as a Windows DLL (tessera_app.dll) in-process.
//
// UNTESTED: written against the documented IIS native module API (httpserv.h)
// without access to Windows/IIS. See README.md. The recommended way to host a
// Tessera application on IIS is HttpPlatformHandler (web.config in this
// directory); use this module only if you need in-process hosting.
//
// Configuration (environment variables of the application pool):
//   TESSERA_APPLICATION  full path of tessera_app.dll
//                        (default: tessera_app.dll next to TesseraModule.dll)
//   TESSERA_MAX_BODY_MB  request body limit in MiB (default 10) -> 413
//   other TESSERA_* variables are read by the application runtime itself.
//
// The application DLL is loaded once per worker process (w3wp.exe) in
// RegisterModule; the ABI version is checked. Failures are written to the
// Windows Application event log (source "TesseraModule") and every request
// then gets a 500.
//
// tessera_handle blocks the IIS request thread until the application returns
// (each call runs the app on its own thread inside the DLL).

#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <sal.h>
#include <httpserv.h>

#include <string.h>
#include <stdlib.h>
#include <new>
#include <string>
#include <vector>

#define TESSERA_NO_PROTOTYPES
#include "tessera.h"

namespace {

// ---- process-wide state (set once in RegisterModule, read-only afterwards) ----

HMODULE g_app = NULL;
tessera_handle_fn g_handle = NULL;
tessera_free_fn g_free = NULL;
ULONGLONG g_max_body = 10ull * 1024 * 1024;

void LogEvent(WORD type, const std::wstring& message)
{
    HANDLE source = RegisterEventSourceW(NULL, L"TesseraModule");
    if (source != NULL) {
        LPCWSTR strings[1] = { message.c_str() };
        ReportEventW(source, type, 0, 1, NULL, 1, 0, strings, NULL);
        DeregisterEventSource(source);
    }
}

std::wstring GetEnv(LPCWSTR name)
{
    wchar_t buffer[32768];
    DWORD n = GetEnvironmentVariableW(name, buffer, ARRAYSIZE(buffer));
    if (n == 0 || n >= ARRAYSIZE(buffer)) {
        return std::wstring();
    }
    return std::wstring(buffer, n);
}

// Directory of TesseraModule.dll, with trailing backslash.
std::wstring ModuleDirectory()
{
    HMODULE self = NULL;
    wchar_t path[MAX_PATH];
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                            reinterpret_cast<LPCWSTR>(&ModuleDirectory), &self)) {
        return std::wstring();
    }
    DWORD n = GetModuleFileNameW(self, path, MAX_PATH);
    if (n == 0 || n >= MAX_PATH) {
        return std::wstring();
    }
    std::wstring dir(path, n);
    size_t slash = dir.find_last_of(L"\\/");
    return slash == std::wstring::npos ? std::wstring() : dir.substr(0, slash + 1);
}

void LoadApplication()
{
    std::wstring path = GetEnv(L"TESSERA_APPLICATION");
    if (path.empty()) {
        path = ModuleDirectory() + L"tessera_app.dll";
    }
    std::wstring maxBody = GetEnv(L"TESSERA_MAX_BODY_MB");
    if (!maxBody.empty()) {
        unsigned long mb = wcstoul(maxBody.c_str(), NULL, 10);
        if (mb > 0 && mb < 4096) {
            g_max_body = static_cast<ULONGLONG>(mb) * 1024 * 1024;
        }
    }

    HMODULE app = LoadLibraryExW(path.c_str(), NULL, LOAD_WITH_ALTERED_SEARCH_PATH);
    if (app == NULL) {
        LogEvent(EVENTLOG_ERROR_TYPE, L"TesseraModule: cannot load " + path + L" (LoadLibrary error " +
                                          std::to_wstring(GetLastError()) + L")");
        return;
    }
    tessera_abi_version_fn version =
        reinterpret_cast<tessera_abi_version_fn>(GetProcAddress(app, "tessera_abi_version"));
    tessera_handle_fn handle = reinterpret_cast<tessera_handle_fn>(GetProcAddress(app, "tessera_handle"));
    tessera_free_fn freeFn = reinterpret_cast<tessera_free_fn>(GetProcAddress(app, "tessera_free"));
    if (version == NULL || handle == NULL || freeFn == NULL) {
        LogEvent(EVENTLOG_ERROR_TYPE, L"TesseraModule: " + path + L" is not a Tessera application library");
        return; // DLL stays loaded; unloading Rust libraries is not reliable
    }
    int32_t v = version();
    if (v != TESSERA_ABI_VERSION) {
        LogEvent(EVENTLOG_ERROR_TYPE, L"TesseraModule: " + path + L" has ABI version " + std::to_wstring(v) +
                                          L", this module supports " + std::to_wstring(TESSERA_ABI_VERSION));
        return;
    }
    g_app = app;
    g_handle = handle;
    g_free = freeFn;
}

// Names of the known request headers, indexed by HTTP_HEADER_ID (http.h).
const char* const kKnownRequestHeaders[] = {
    "cache-control", "connection", "date", "keep-alive", "pragma", "trailer", "transfer-encoding",
    "upgrade", "via", "warning", "allow", "content-length", "content-type", "content-encoding",
    "content-language", "content-location", "content-md5", "content-range", "expires", "last-modified",
    "accept", "accept-charset", "accept-encoding", "accept-language", "authorization", "cookie", "expect",
    "from", "host", "if-match", "if-modified-since", "if-none-match", "if-range", "if-unmodified-since",
    "max-forwards", "proxy-authorization", "referer", "range", "te", "translate", "user-agent",
};
static_assert(ARRAYSIZE(kKnownRequestHeaders) == HttpHeaderRequestMaximum, "known header table out of date");

void PutU32(std::vector<uint8_t>& out, uint32_t v)
{
    uint8_t b[4];
    tessera_put_u32(b, v);
    out.insert(out.end(), b, b + 4);
}

void PutBytes(std::vector<uint8_t>& out, const void* data, size_t n)
{
    PutU32(out, static_cast<uint32_t>(n));
    const uint8_t* p = static_cast<const uint8_t*>(data);
    out.insert(out.end(), p, p + n);
}

void PutLower(std::vector<uint8_t>& out, const char* data, size_t n)
{
    PutU32(out, static_cast<uint32_t>(n));
    for (size_t i = 0; i < n; i++) {
        char c = data[i];
        out.push_back(static_cast<uint8_t>((c >= 'A' && c <= 'Z') ? c + ('a' - 'A') : c));
    }
}

std::string RemoteAddress(const HTTP_REQUEST* raw)
{
    char host[NI_MAXHOST] = "";
    PSOCKADDR sa = raw->Address.pRemoteAddress;
    if (sa == NULL) {
        return std::string();
    }
    int len = sa->sa_family == AF_INET6 ? sizeof(SOCKADDR_IN6) : sizeof(SOCKADDR_IN);
    if (getnameinfo(sa, len, host, sizeof host, NULL, 0, NI_NUMERICHOST) != 0) {
        return std::string();
    }
    return std::string(host);
}

const char* ReasonPhrase(USHORT status)
{
    switch (status) {
    case 200: return "OK";
    case 201: return "Created";
    case 204: return "No Content";
    case 301: return "Moved Permanently";
    case 302: return "Found";
    case 303: return "See Other";
    case 304: return "Not Modified";
    case 307: return "Temporary Redirect";
    case 308: return "Permanent Redirect";
    case 400: return "Bad Request";
    case 401: return "Unauthorized";
    case 403: return "Forbidden";
    case 404: return "Not Found";
    case 405: return "Method Not Allowed";
    case 409: return "Conflict";
    case 413: return "Payload Too Large";
    case 415: return "Unsupported Media Type";
    case 422: return "Unprocessable Content";
    case 429: return "Too Many Requests";
    case 500: return "Internal Server Error";
    case 502: return "Bad Gateway";
    case 503: return "Service Unavailable";
    case 504: return "Gateway Timeout";
    default: return "";
    }
}

bool IsSkippedResponseHeader(const char* name, size_t n)
{
    static const char* const skip[] = { "content-length", "transfer-encoding", "connection",
                                        "keep-alive", "upgrade", "status" };
    for (size_t i = 0; i < ARRAYSIZE(skip); i++) {
        if (strlen(skip[i]) == n && _strnicmp(skip[i], name, n) == 0) {
            return true;
        }
    }
    return false;
}

REQUEST_NOTIFICATION_STATUS SimpleError(IHttpResponse* response, USHORT status)
{
    response->Clear();
    response->SetStatus(status, ReasonPhrase(status));
    return RQ_NOTIFICATION_FINISH_REQUEST;
}

// Frees the library's response buffer on every exit path.
struct ResponseBuffer {
    uint8_t* data = NULL;
    size_t len = 0;
    ~ResponseBuffer() { if (data != NULL) g_free(data, len); }
};

class CTesseraModule final : public CHttpModule {
public:
    REQUEST_NOTIFICATION_STATUS OnExecuteRequestHandler(IHttpContext* context, IHttpEventProvider* provider) override
    {
        IHttpRequest* request = context->GetRequest();
        IHttpResponse* response = context->GetResponse();

        if (g_handle == NULL) {
            return SimpleError(response, 500); // reason logged once at startup
        }

        // ---- request body ----
        std::vector<uint8_t> body;
        DWORD remaining = request->GetRemainingEntityBytes();
        if (remaining > g_max_body) {
            return SimpleError(response, 413);
        }
        if (remaining > 0) {
            body.reserve(remaining);
        }
        uint8_t chunk[16 * 1024];
        for (;;) {
            DWORD read = 0;
            HRESULT hr = request->ReadEntityBody(chunk, sizeof chunk, FALSE, &read, NULL);
            if (hr == HRESULT_FROM_WIN32(ERROR_HANDLE_EOF)) {
                break;
            }
            if (FAILED(hr)) {
                return SimpleError(response, 400);
            }
            if (body.size() + read > g_max_body) {
                return SimpleError(response, 413);
            }
            if (read == 0) {
                break;
            }
            body.insert(body.end(), chunk, chunk + read);
        }

        // ---- wire request ----
        const HTTP_REQUEST* raw = request->GetRawHttpRequest();
        const char* url = raw->pRawUrl != NULL ? raw->pRawUrl : "/";
        size_t urlLen = raw->pRawUrl != NULL ? raw->RawUrlLength : 1;
        // absolute-form "http://host/path": skip scheme and authority
        if (urlLen > 0 && url[0] != '/') {
            const char* scheme = static_cast<const char*>(memchr(url, ':', urlLen));
            if (scheme != NULL && static_cast<size_t>(scheme - url) + 3 <= urlLen && scheme[1] == '/' && scheme[2] == '/') {
                const char* authority = scheme + 3;
                const char* end = url + urlLen;
                const char* slash = static_cast<const char*>(memchr(authority, '/', end - authority));
                if (slash != NULL) {
                    urlLen = end - slash;
                    url = slash;
                }
            }
        }
        const char* q = static_cast<const char*>(memchr(url, '?', urlLen));
        size_t pathLen = q != NULL ? static_cast<size_t>(q - url) : urlLen;
        const char* query = q != NULL ? q + 1 : "";
        size_t queryLen = q != NULL ? urlLen - pathLen - 1 : 0;
        const char* method = request->GetHttpMethod();
        std::string remote = RemoteAddress(raw);

        std::vector<uint8_t> wire;
        wire.reserve(body.size() + 4096);
        PutU32(wire, TESSERA_WIRE_VERSION);
        PutBytes(wire, method, strlen(method));
        PutBytes(wire, url, pathLen);
        PutBytes(wire, query, queryLen);
        PutBytes(wire, remote.data(), remote.size());

        const HTTP_REQUEST_HEADERS& headers = raw->Headers;
        uint32_t count = 0;
        for (int i = 0; i < HttpHeaderRequestMaximum; i++) {
            if (headers.KnownHeaders[i].pRawValue != NULL) count++;
        }
        for (USHORT i = 0; i < headers.UnknownHeaderCount; i++) {
            if (headers.pUnknownHeaders[i].pName != NULL && headers.pUnknownHeaders[i].pRawValue != NULL) count++;
        }
        PutU32(wire, count);
        for (int i = 0; i < HttpHeaderRequestMaximum; i++) {
            const HTTP_KNOWN_HEADER& h = headers.KnownHeaders[i];
            if (h.pRawValue != NULL) {
                PutBytes(wire, kKnownRequestHeaders[i], strlen(kKnownRequestHeaders[i]));
                PutBytes(wire, h.pRawValue, h.RawValueLength);
            }
        }
        for (USHORT i = 0; i < headers.UnknownHeaderCount; i++) {
            const HTTP_UNKNOWN_HEADER& h = headers.pUnknownHeaders[i];
            if (h.pName != NULL && h.pRawValue != NULL) {
                PutLower(wire, h.pName, h.NameLength);
                PutBytes(wire, h.pRawValue, h.RawValueLength);
            }
        }
        PutBytes(wire, body.data(), body.size());
        std::vector<uint8_t>().swap(body);

        // ---- call the application ----
        ResponseBuffer out;
        if (g_handle(wire.data(), wire.size(), &out.data, &out.len) != TESSERA_OK || out.data == NULL) {
            return SimpleError(response, 500);
        }

        // ---- response ----
        tessera_reader rd;
        tessera_reader_init(&rd, out.data, out.len);
        uint32_t version = tessera_read_u32(&rd);
        uint32_t status = tessera_read_u32(&rd);
        uint32_t headerCount = tessera_read_u32(&rd);
        if (!rd.ok || version != TESSERA_WIRE_VERSION || status < 100 || status > 999) {
            return SimpleError(response, 500);
        }

        response->Clear();
        response->SetStatus(static_cast<USHORT>(status), ReasonPhrase(static_cast<USHORT>(status)));
        for (uint32_t i = 0; i < headerCount; i++) {
            size_t nl = 0, vl = 0;
            const uint8_t* name = tessera_read_bytes(&rd, &nl);
            const uint8_t* value = tessera_read_bytes(&rd, &vl);
            if (!rd.ok || vl > 0xFFFF) {
                return SimpleError(response, 500);
            }
            if (IsSkippedResponseHeader(reinterpret_cast<const char*>(name), nl)) {
                continue;
            }
            std::string n(reinterpret_cast<const char*>(name), nl);
            std::string v(reinterpret_cast<const char*>(value), vl);
            // fReplace = FALSE so repeated headers (Set-Cookie) are all sent.
            HRESULT hr = response->SetHeader(n.c_str(), v.c_str(), static_cast<USHORT>(v.size()), FALSE);
            if (FAILED(hr)) {
                return SimpleError(response, 500);
            }
        }
        size_t bodyLen = 0;
        const uint8_t* responseBody = tessera_read_bytes(&rd, &bodyLen);
        if (!rd.ok || bodyLen > 0xFFFFFFFFu) {
            return SimpleError(response, 500);
        }

        if (bodyLen > 0) {
            // Copy into request-lifetime memory: IIS may keep referencing the
            // chunk after WriteEntityChunks returns (response buffering).
            void* copy = context->AllocateRequestMemory(static_cast<DWORD>(bodyLen));
            if (copy == NULL) {
                return SimpleError(response, 500);
            }
            memcpy(copy, responseBody, bodyLen);
            HTTP_DATA_CHUNK dataChunk;
            dataChunk.DataChunkType = HttpDataChunkFromMemory;
            dataChunk.FromMemory.pBuffer = copy;
            dataChunk.FromMemory.BufferLength = static_cast<ULONG>(bodyLen);
            DWORD sent = 0;
            HRESULT hr = response->WriteEntityChunks(&dataChunk, 1, FALSE, FALSE, &sent);
            if (FAILED(hr)) {
                provider->SetErrorStatus(hr);
                return RQ_NOTIFICATION_FINISH_REQUEST;
            }
        }
        return RQ_NOTIFICATION_FINISH_REQUEST;
    }
};

class CTesseraModuleFactory final : public IHttpModuleFactory {
public:
    HRESULT GetHttpModule(CHttpModule** module, IModuleAllocator* allocator) override
    {
        UNREFERENCED_PARAMETER(allocator);
        CTesseraModule* m = new (std::nothrow) CTesseraModule();
        if (m == NULL) {
            return HRESULT_FROM_WIN32(ERROR_NOT_ENOUGH_MEMORY);
        }
        *module = m;
        return S_OK;
    }

    void Terminate() override { delete this; }
};

} // namespace

// Exported (see TesseraModule.def); called once when w3wp.exe loads the module.
HRESULT __stdcall RegisterModule(DWORD serverVersion, IHttpModuleRegistrationInfo* moduleInfo, IHttpServer* server)
{
    UNREFERENCED_PARAMETER(serverVersion);
    UNREFERENCED_PARAMETER(server);
    LoadApplication();
    CTesseraModuleFactory* factory = new (std::nothrow) CTesseraModuleFactory();
    if (factory == NULL) {
        return HRESULT_FROM_WIN32(ERROR_NOT_ENOUGH_MEMORY);
    }
    return moduleInfo->SetRequestNotifications(factory, RQ_EXECUTE_REQUEST_HANDLER, 0);
}
