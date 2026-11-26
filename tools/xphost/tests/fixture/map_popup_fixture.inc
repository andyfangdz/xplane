// Asset-free, independent C ABI consumer. Each command runs in the plugin,
// while the host script changes headset state and captures surface commands.
namespace map_popup_fixture {
XPLMMapDisplayRef pilot{}, copilot{};
XPLMAvionicsID screen{}, stock{};
XPLMMapDrawInfo_t rect{sizeof(rect), xplm_Map_Topo | xplm_Map_Water | xplm_Map_safe_taxi,
                       10, 150, 210, 50};
XPLMMapCustomData_t custom{};

void check_unavailable(XPLMMapDisplayRef map) {
    float x = 41, y = 42;
    double lat = 91, lon = 181;
    Check("project preserves output on failure",
          !XPLMMapDisplayProject(map, &rect, &custom, 0, 0, &x, &y) && x == 41 && y == 42);
    Check("unproject preserves output on failure",
          !XPLMMapDisplayUnproject(map, &rect, &custom, 20, 30, &lat, &lon) && lat == 91 && lon == 181);
}

void draw(void*) {
    XPLMTransformPush();
    XPLMTransformTranslate(12, 9);
    XPLMScissorPush();
    XPLMScissorIntersect(5, 0, 195, 132);
    XPLMMapDisplayDrawIn(pilot, &rect, &custom);
    XPLMScissorPop();
    XPLMTransformPop();
    if (copilot) XPLMMapDisplayDrawIn(copilot, &rect, nullptr);
    float low = -11, high = 800;
    Check("no invented elevation data",
          !XPLMMapDisplayGetTerrainAltitudes(pilot, &low, &high) && low == -11 && high == 800);
}

int start(XPLMCommandRef, XPLMCommandPhase phase, void*) {
    if (phase != xplm_CommandBegin || screen) return 1;
    XPLMCreateMap_t params{sizeof(params), 0};
    pilot = XPLMCreateMapDisplay(&params);
    Check("pilot map created", pilot != nullptr);
    custom.structSize = sizeof(custom);
    custom.datLat = 47.5f;
    custom.datLon = 179.995f;
    custom.centerX = 60;
    custom.centerY = 78;
    custom.roseRadius = 45;
    custom.mapRange = 3;
    custom.orientation = 3;
    custom.trueRotation = 90;
    check_unavailable(pilot);
    params.structSize--;
    Check("bad map record rejected", !XPLMCreateMapDisplay(&params));

    XPLMCreateAvionics_t av{};
    av.structSize = sizeof(av);
    av.deviceID = "org.xphost.map";
    av.deviceName = "Synthetic Map";
    av.screenWidth = 240; av.screenHeight = 180;
    av.drawCallback = draw;
    av.contentType = xplm_WindowContentTypePanelGraphics;
    screen = XPLMCreateAvionicsEx(&av);
    XPLMCustomizeAvionics_t built{};
    built.structSize = sizeof(built);
    built.deviceId = xplm_device_G1000_MFD;
    stock = XPLMRegisterAvionicsCallbacksEx(&built);
    XPLMSetAvionicsMappedToVR(screen, 1);
    Check("VR mapping ignored before headset start", !XPLMIsAvionicsMappedToVR(screen));
    return 1;
}

int probe(XPLMCommandRef, XPLMCommandPhase phase, void*) {
    if (phase != xplm_CommandBegin) return 1;
    float x = 0, y = 0;
    Check("project datum at untransformed off-center panel origin",
          XPLMMapDisplayProject(pilot, &rect, &custom, custom.datLat, custom.datLon, &x, &y) &&
          x == custom.centerX && y == custom.centerY);
    Check("true north faces left on an east-up map",
          std::abs(XPLMMapDisplayGetNorthHeading(pilot, &rect, &custom, x, y) - 270.f) < 0.001f);
    Check("map range is radius in nautical miles",
          std::abs(XPLMMapDisplayScaleMeter(pilot, &rect, &custom, x, y) - 45.f / (3 * 1852)) < 0.000001f);
    double lat = 0, lon = 0;
    Check("east across date line points up",
          XPLMMapDisplayProject(pilot, &rect, &custom, 47.5, -179.995, &x, &y) && y > 80 && std::abs(x - 60) < 0.1);
    Check("touch inverts across date line outside draw callback",
          XPLMMapDisplayUnproject(pilot, &rect, &custom, x, y, &lat, &lon) &&
          std::abs(lat - 47.5) < 0.000001 && std::abs(lon + 179.995) < 0.000001);
    XPLMCreateMap_t params{sizeof(params), 1};
    copilot = XPLMCreateMapDisplay(&params);
    Check("independent copilot handle", copilot && copilot != pilot);
    check_unavailable(copilot);
    XPLMSetAvionicsMappedToVR(stock, 1);
    Check("stock popup stays on desktop", !XPLMIsAvionicsMappedToVR(stock));
    XPLMPopOutAvionics(screen);
    Check("popout remains hidden", XPLMIsAvionicsPoppedOut(screen) && !XPLMIsAvionicsPopupVisible(screen));
    XPLMSetAvionicsMappedToVR(screen, 1);
    Check("mapping hidden popout selects VR",
          !XPLMIsAvionicsPoppedOut(screen) && XPLMIsAvionicsMappedToVR(screen) && !XPLMIsAvionicsPopupVisible(screen));
    XPLMSetAvionicsPopupVisible(screen, 1);
    XPLMSetAvionicsPopupVisible(screen, 0);
    Check("hiding preserves VR placement", XPLMIsAvionicsMappedToVR(screen));
    XPLMSetAvionicsPopupVisible(screen, 1);
    XPLMPopOutAvionics(screen);
    Check("popout clears VR", XPLMIsAvionicsPoppedOut(screen) && !XPLMIsAvionicsMappedToVR(screen));
    XPLMSetAvionicsMappedToVR(screen, 1);
    return 1;
}

void shutdown() {
    if (!screen) return;
    XPLMDestroyMapDisplay(pilot);
    if (copilot) XPLMDestroyMapDisplay(copilot);
    check_unavailable(pilot);
    XPLMUnregisterAvionicsCallbacks(stock);
    XPLMDestroyAvionics(screen);
    pilot = copilot = nullptr;
    screen = stock = nullptr;
}
int finish(XPLMCommandRef, XPLMCommandPhase phase, void*) {
    if (phase == xplm_CommandBegin) shutdown();
    return 1;
}
} // namespace map_popup_fixture
