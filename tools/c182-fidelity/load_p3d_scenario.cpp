// Loads a separate test scenario using the documented SimConnect API.
// The observer is deliberately a separate, read-only executable.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <SimConnect.h>
#include <chrono>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <string>

template<class T> T symbol(HMODULE module, const char* name) {
    const auto address = GetProcAddress(module, name);
    if (!address) throw std::runtime_error(std::string("Missing API: ") + name);
    static_assert(sizeof(T) == sizeof(address));
    T result; std::memcpy(&result, &address, sizeof(result)); return result;
}

int main(int argc, char** argv) {
    if (argc != 3) {
        std::cerr << "Usage: load_p3d_scenario SimConnect.dll scenario.fxml\n";
        return 2;
    }
    if (GetFileAttributesA(argv[2]) == INVALID_FILE_ATTRIBUTES) {
        std::cerr << "Scenario not found\n"; return 2;
    }
    HMODULE module = nullptr;
    HANDLE connection = nullptr;
    decltype(&SimConnect_Close) close = nullptr;
    try {
        module = LoadLibraryExA(argv[1], nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
        if (!module) throw std::runtime_error("Cannot load SimConnect");
        const auto open = symbol<decltype(&SimConnect_Open)>(module, "SimConnect_Open");
        close = symbol<decltype(&SimConnect_Close)>(module, "SimConnect_Close");
        const auto load = symbol<decltype(&SimConnect_FlightLoad)>(module, "SimConnect_FlightLoad");
        const auto dispatch = symbol<decltype(&SimConnect_GetNextDispatch)>(module, "SimConnect_GetNextDispatch");
        if (FAILED(open(&connection, "C182 reference scenario loader", nullptr, 0, nullptr, 0)))
            throw std::runtime_error("Cannot connect to running simulator");
        bool sent = false;
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(15);
        while (std::chrono::steady_clock::now() < deadline) {
            SIMCONNECT_RECV* data = nullptr;
            DWORD size = 0;
            if (FAILED(dispatch(connection, &data, &size))) { Sleep(10); continue; }
            if (!data || size < sizeof(SIMCONNECT_RECV)) throw std::runtime_error("Malformed message");
            if (data->dwID == SIMCONNECT_RECV_ID_OPEN && !sent) {
                if (size < sizeof(SIMCONNECT_RECV_OPEN)) throw std::runtime_error("Malformed OPEN");
                const auto* server = reinterpret_cast<const SIMCONNECT_RECV_OPEN*>(data);
                const std::string name(server->szApplicationName, strnlen_s(server->szApplicationName, 256));
                if (name.find("Prepar3D") == std::string::npos || server->dwApplicationVersionMajor != 5)
                    throw std::runtime_error("Expected Prepar3D v5; refusing to load scenario in another simulator");
                if (FAILED(load(connection, argv[2]))) throw std::runtime_error("FlightLoad request failed");
                sent = true;
                std::cout << "Scenario load requested; aircraft title and loaded state still require verification.\n";
            }
            if (data->dwID == SIMCONNECT_RECV_ID_EXCEPTION) {
                if (size < sizeof(SIMCONNECT_RECV_EXCEPTION)) throw std::runtime_error("Malformed EXCEPTION");
                const auto* e = reinterpret_cast<const SIMCONNECT_RECV_EXCEPTION*>(data);
                throw std::runtime_error("SimConnect exception " + std::to_string(e->dwException));
            }
            if (data->dwID == SIMCONNECT_RECV_ID_QUIT) throw std::runtime_error("Simulator quit");
        }
        close(connection); connection = nullptr;
        FreeLibrary(module); module = nullptr;
        if (!sent) throw std::runtime_error("Timed out before OPEN handshake");
        return 0;
    } catch (const std::exception& error) {
        if (connection && close) close(connection);
        if (module) FreeLibrary(module);
        std::cerr << error.what() << '\n'; return 1;
    }
}
