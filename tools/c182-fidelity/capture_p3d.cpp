// Read-only observer using the published SimConnect API. No gauge injection,
// aircraft edits, flight-state writes, or proprietary simulation code.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <SimConnect.h>
#include <chrono>
#include <cmath>
#include <cstring>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <stdexcept>
#include <string>

struct Field { const char* key; const char* variable; const char* unit; };
static const Field fields[] = {
    {"sim_time_s", "ABSOLUTE TIME", "seconds"},
    {"sim_rate", "SIMULATION RATE", "number"},
    {"on_ground", "SIM ON GROUND", "bool"},
    {"pressure_altitude_ft", "PRESSURE ALTITUDE", "feet"},
    {"altitude_msl_ft", "PLANE ALTITUDE", "feet"},
    {"oat_c", "AMBIENT TEMPERATURE", "celsius"},
    {"ambient_pressure_inhg", "AMBIENT PRESSURE", "inHg"},
    {"wind_kt", "AMBIENT WIND VELOCITY", "knots"},
    {"weight_lb", "TOTAL WEIGHT", "pounds"},
    {"cg_pct", "CG PERCENT", "percent"},
    {"ias_kt", "AIRSPEED INDICATED", "knots"},
    {"tas_kt", "AIRSPEED TRUE", "knots"},
    {"vs_fpm", "VERTICAL SPEED", "feet per minute"},
    {"bank_deg", "PLANE BANK DEGREES", "degrees"},
    {"pitch_deg", "PLANE PITCH DEGREES", "degrees"},
    {"aoa_deg", "INCIDENCE ALPHA", "degrees"},
    {"rpm", "GENERAL ENG RPM:1", "rpm"},
    {"map_inhg", "ENG MANIFOLD PRESSURE:1", "inHg"},
    {"fuel_flow_gph", "ENG FUEL FLOW GPH:1", "gallons per hour"},
    {"cht_c", "RECIP ENG CYLINDER HEAD TEMPERATURE:1", "celsius"},
    {"egt_c", "GENERAL ENG EXHAUST GAS TEMPERATURE:1", "celsius"},
    {"oil_c", "GENERAL ENG OIL TEMPERATURE:1", "celsius"},
    {"oil_psi", "GENERAL ENG OIL PRESSURE:1", "psi"},
    {"fuel_psi", "GENERAL ENG FUEL PRESSURE:1", "psi"},
    {"throttle_pct", "GENERAL ENG THROTTLE LEVER POSITION:1", "percent"},
    {"mixture_pct", "GENERAL ENG MIXTURE LEVER POSITION:1", "percent"},
    {"prop_lever_pct", "GENERAL ENG PROPELLER LEVER POSITION:1", "percent"},
    {"cowl_flaps_pct", "RECIP ENG COWL FLAP POSITION:1", "percent"},
    {"flap_deg", "TRAILING EDGE FLAPS LEFT ANGLE", "degrees"},
    {"fuel_left_gal", "FUEL TANK LEFT MAIN QUANTITY", "gallons"},
    {"fuel_right_gal", "FUEL TANK RIGHT MAIN QUANTITY", "gallons"},
    {"bus_v", "ELECTRICAL MAIN BUS VOLTAGE", "volts"},
    {"battery_v", "ELECTRICAL BATTERY BUS VOLTAGE", "volts"},
    {"battery_a", "ELECTRICAL BATTERY LOAD", "amperes"},
    {"combustion", "GENERAL ENG COMBUSTION:1", "bool"},
    {"starter", "GENERAL ENG STARTER:1", "bool"},
    {"left_mag", "RECIP ENG LEFT MAGNETO:1", "bool"},
    {"right_mag", "RECIP ENG RIGHT MAGNETO:1", "bool"},
};

static std::string quoted(const char* text) {
    std::string result = "\"";
    for (const unsigned char* p = reinterpret_cast<const unsigned char*>(text); *p; ++p) {
        if (*p == '\\' || *p == '"') result += '\\';
        if (*p < 32 || *p >= 128) {
            constexpr char hex[] = "0123456789abcdef";
            result += "\\u00"; result += hex[*p >> 4]; result += hex[*p & 15];
        } else result += static_cast<char>(*p);
    }
    return result + '"';
}

template<class T> T symbol(HMODULE module, const char* name) {
    const auto address = GetProcAddress(module, name);
    if (!address) throw std::runtime_error(std::string("Missing API: ") + name);
    // memcpy avoids MSVC's incompatible-function-pointer cast diagnostic.
    static_assert(sizeof(T) == sizeof(address));
    T result;
    std::memcpy(&result, &address, sizeof(result));
    return result;
}

static void checked(HRESULT hr, const char* operation) {
    if (FAILED(hr)) throw std::runtime_error(std::string(operation) + " failed: " + std::to_string(hr));
}

int main(int argc, char** argv) {
    if (argc != 4) {
        std::cerr << "Usage: capture_p3d SimConnect.dll output.jsonl duration_seconds\n";
        return 2;
    }
    HMODULE module = nullptr;
    HANDLE connection = nullptr;
    decltype(&SimConnect_Close) close = nullptr;
    try {
        const double duration = std::stod(argv[3]);
        if (!std::isfinite(duration) || duration < 1 || duration > 3600)
            throw std::runtime_error("Duration must be between 1 and 3600 seconds");
        // Fail rather than overwrite an existing evidence file.
        if (GetFileAttributesA(argv[2]) != INVALID_FILE_ATTRIBUTES)
            throw std::runtime_error("Output already exists");
        std::ofstream output(argv[2]);
        if (!output) throw std::runtime_error("Cannot create output");
        output << std::setprecision(15);
        module = LoadLibraryExA(argv[1], nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
        if (!module) throw std::runtime_error("Cannot load SimConnect DLL: Win32 " + std::to_string(GetLastError()));
        const auto open = symbol<decltype(&SimConnect_Open)>(module, "SimConnect_Open");
        close = symbol<decltype(&SimConnect_Close)>(module, "SimConnect_Close");
        const auto define = symbol<decltype(&SimConnect_AddToDataDefinition)>(module, "SimConnect_AddToDataDefinition");
        const auto request = symbol<decltype(&SimConnect_RequestDataOnSimObject)>(module, "SimConnect_RequestDataOnSimObject");
        const auto dispatch = symbol<decltype(&SimConnect_GetNextDispatch)>(module, "SimConnect_GetNextDispatch");
        const auto subscribe = symbol<decltype(&SimConnect_SubscribeToSystemEvent)>(module, "SimConnect_SubscribeToSystemEvent");
        checked(open(&connection, "C182 read-only fidelity observer", nullptr, 0, nullptr, 0), "Open");
        checked(subscribe(connection, 10, "Pause"), "Subscribe Pause");
        checked(subscribe(connection, 11, "Sim"), "Subscribe Sim");
        for (const auto& f : fields)
            checked(define(connection, 1, f.variable, f.unit, SIMCONNECT_DATATYPE_FLOAT64, 0, SIMCONNECT_UNUSED), f.variable);
        checked(define(connection, 1, "TITLE", nullptr, SIMCONNECT_DATATYPE_STRING256, 0, SIMCONNECT_UNUSED), "TITLE");
        checked(request(connection, 1, 1, SIMCONNECT_OBJECT_ID_USER, SIMCONNECT_PERIOD_ONCE, 0, 0, 0, 0), "Request samples");
        output << "{\"type\":\"metadata\",\"schema\":1,\"observer\":\"simconnect_native_vars\","
                  "\"accepted_baseline\":false,\"warning\":\"Native simulator variables require gauge cross-check; private A2A states are not captured\"}\n";
        const auto start = std::chrono::steady_clock::now();
        int paused = -1, running = -1;
        size_t samples = 0, errors = 0;
        double next_request_s = 0.1;
        bool quit = false;
        while (!quit) {
            const double elapsed = std::chrono::duration<double>(std::chrono::steady_clock::now() - start).count();
            if (elapsed >= duration) break;
            if (elapsed >= next_request_s) {
                checked(request(connection, 1, 1, SIMCONNECT_OBJECT_ID_USER, SIMCONNECT_PERIOD_ONCE, 0, 0, 0, 0), "Request sample");
                next_request_s = elapsed + 0.1;
            }
            SIMCONNECT_RECV* data = nullptr;
            DWORD size = 0;
            if (FAILED(dispatch(connection, &data, &size))) { Sleep(5); continue; }
            if (!data || size < sizeof(SIMCONNECT_RECV) || data->dwSize > size) {
                throw std::runtime_error("Invalid SimConnect message length");
            }
            switch (data->dwID) {
            case SIMCONNECT_RECV_ID_OPEN: {
                if (size < sizeof(SIMCONNECT_RECV_OPEN)) throw std::runtime_error("Truncated OPEN");
                const auto* p = reinterpret_cast<const SIMCONNECT_RECV_OPEN*>(data);
                char app[257]{}; std::memcpy(app, p->szApplicationName, 256);
                output << "{\"type\":\"server\",\"name\":" << quoted(app)
                       << ",\"major\":" << p->dwApplicationVersionMajor << ",\"minor\":" << p->dwApplicationVersionMinor
                       << ",\"build\":" << p->dwApplicationBuildMajor << "}\n";
                break;
            }
            case SIMCONNECT_RECV_ID_EXCEPTION: {
                if (size < sizeof(SIMCONNECT_RECV_EXCEPTION)) throw std::runtime_error("Truncated EXCEPTION");
                const auto* p = reinterpret_cast<const SIMCONNECT_RECV_EXCEPTION*>(data);
                output << "{\"type\":\"exception\",\"code\":" << p->dwException << ",\"send_id\":" << p->dwSendID
                       << ",\"index\":" << p->dwIndex << "}\n";
                ++errors;
                break;
            }
            case SIMCONNECT_RECV_ID_EVENT: {
                if (size < sizeof(SIMCONNECT_RECV_EVENT)) throw std::runtime_error("Truncated EVENT");
                const auto* p = reinterpret_cast<const SIMCONNECT_RECV_EVENT*>(data);
                if (p->uEventID == 10) paused = static_cast<int>(p->dwData);
                if (p->uEventID == 11) running = static_cast<int>(p->dwData);
                break;
            }
            case SIMCONNECT_RECV_ID_SIMOBJECT_DATA: {
                constexpr size_t header = sizeof(SIMCONNECT_RECV_SIMOBJECT_DATA) - sizeof(DWORD);
                if (size < header) throw std::runtime_error("Truncated DATA");
                const auto* p = reinterpret_cast<const SIMCONNECT_RECV_SIMOBJECT_DATA*>(data);
                const auto* payload = reinterpret_cast<const unsigned char*>(data) + header;
                if (p->dwRequestID == 1) {
                    constexpr size_t count = sizeof(fields) / sizeof(fields[0]);
                    if (p->dwDefineCount != count + 1 || size < header + count * sizeof(double) + 256)
                        throw std::runtime_error("Unexpected sample layout");
                    char title[257]{}; std::memcpy(title, payload + count * sizeof(double), 256);
                    output << "{\"type\":\"sample\",\"wall_elapsed_s\":" << elapsed
                           << ",\"title\":" << quoted(title) << ",\"paused\":" << paused << ",\"running\":" << running;
                    for (size_t i = 0; i < count; ++i) {
                        double value; std::memcpy(&value, payload + i * sizeof(double), sizeof(value));
                        output << ',' << quoted(fields[i].key) << ':';
                        if (std::isfinite(value)) output << value;
                        else { output << "null"; ++errors; }
                    }
                    output << "}\n";
                    ++samples;
                }
                break;
            }
            case SIMCONNECT_RECV_ID_QUIT: quit = true; break;
            default: break;
            }
        }
        output << "{\"type\":\"end\",\"samples\":" << samples << ",\"errors\":" << errors << ",\"simulator_quit\":" << (quit ? "true" : "false") << "}\n";
        output.flush();
        if (!output) throw std::runtime_error("Failed writing evidence");
        checked(close(connection), "Close"); connection = nullptr;
        FreeLibrary(module); module = nullptr;
        std::cout << "Captured " << samples << " samples, " << errors << " errors. Not an accepted baseline.\n";
        return samples && !errors && !quit ? 0 : 1;
    } catch (const std::exception& error) {
        if (connection && close) close(connection);
        if (module) FreeLibrary(module);
        std::cerr << error.what() << '\n';
        return 1;
    }
}
