// Independent ABI consumer, compiled against the public SDK headers.
namespace panel_fixture {
XPLMWindowID window{};
XPLMAvionicsID device{};
XPLMAvionicsID builtin{};
XPLMFontHandle font{};
XPLMTextureAtlasRef atlas{};
XPLMInstanceRef instance{};
int drawn = 0;

void draw_window(XPLMWindowID id, void*) {
    FaultAt("draw");
    XPLMVertex_t quad[] = {{30, 40}, {42, 40}, {42, 50}, {30, 50}};
    XPLMPolygon(0xff010101, quad, 4);
    const unsigned char pixels[] = {12, 34, 56, 255};
    auto texture = XPLMCreateTexture(pixels, 1, 1);
    struct Vertex { float x, y, u, v; uint32_t color; };
    const Vertex vertices[] = {{0, 0, 0, 0, 0}, {1, 2, 0, 0, 0xff302010},
                               {5, 2, 1, 0, 0xffffffff}, {1, 5, 0, 1, 0xffffffff}};
    const uint16_t indices[] = {999, 0, 1, 2};
    XPLMMesh_t mesh{4, reinterpret_cast<const float*>(vertices), 4, indices};
    XPLMDrawCall_t call{texture, {1, 2, 6, 6}, 1, 3, 1};
    XPLMTransformPush();
    XPLMTransformTranslate(10, 50);
    XPLMTransformScale(2, -2);
    XPLMScissorPush();
    XPLMScissorIntersect(0, 0, 4, 10);
    XPLMDrawCalls(&mesh, 1, &call);
    if (std::getenv("XPHOST_FIXTURE_BAD_INDEX")) {
        auto invalid = call;
        invalid.vtx_offset = 2;
        XPLMDrawCalls(&mesh, 1, &invalid);
    }
    XPLMScissorPop();
    XPLMTransformPop();
    XPLMDestroyTexture(texture);
    if (std::getenv("XPHOST_FIXTURE_STALE_TEXTURE")) XPLMDrawCalls(&mesh, 1, &call);
    XPLMFontDrawStringWordWrapped(font, 0xffffffff, 14, 30, 49, "Cockpit", 80, xplm_JustLeft);
    XPLMTextureAtlasDrawIn(atlas, 1, 0xffffffff, 35, 48, 36, 47);
    ++drawn;
    if (drawn == 1) XPLMSetWindowIsVisible(id, 0);
}
void draw_screen(void*) {
    const XPLMVertex_t triangle[] = {{0, 0}, {2, 0}, {0, 2}};
    XPLMPolygon(0xff00ffff, triangle, 3);
}
int draw_builtin(XPLMDeviceID, int before, void*) {
    const XPLMVertex_t line[] = {{0, 0}, {1, 1}};
    XPLMLinesWithWidth(before ? 0xff0000ff : 0x00ff00ff, 1, line, 2);
    return 1;
}
int open(XPLMCommandRef, XPLMCommandPhase phase, void*) {
    if (phase != xplm_CommandBegin) return 1;
    if (window) {
        XPLMSetWindowIsVisible(window, 1);
        XPLMAvionicsNeedsDrawing(device);
        return 1;
    }
    font = XPLMCreateFont(xplm_CharSetUnicode);
    Check("invalid font rejected", XPLMFontAddFace(font, "xphost-missing.ttf") == 0);
    Check("load fixture font", XPLMFontAddFace(font, "Resources/fonts/fixture.ttf") == 1);
    const float wide = XPLMFontMeasureString(font, 14, "WWW");
    const float narrow = XPLMFontMeasureString(font, 14, "iii");
    Check("face glyph metrics", wide > narrow && narrow > 0);

    atlas = XPLMCreateTextureAtlas();
    const unsigned char pixels[] = {255, 0, 0, 255, 0, 255, 0, 255};
    Check("atlas first cell", XPLMTextureAtlasAddImageSet(atlas, pixels, 2, 1, 2, 1) == 0);
    XPLMTextureAtlasBake(atlas);
    Check("baked atlas is sealed", XPLMTextureAtlasAddImage(atlas, pixels, 1, 1) == -1);

    const char* datarefs[] = {"xphost/test/value", nullptr};
    auto object = XPLMLoadObject("./fixture.obj");
    Check("instance rejects null object", !XPLMCreateInstance(nullptr, datarefs));
    Check("instance requires dataref list", !XPLMCreateInstance(object, nullptr));
    instance = XPLMCreateInstance(object, datarefs);
    Check("instance created", object && instance);
    XPLMUnloadObject(object);
    Check("unloaded object rejected", !XPLMCreateInstance(object, datarefs));
    XPLMDrawInfo_t pose{sizeof(XPLMDrawInfo_t), 1, 2, 3, 4, 5, 6};
    const float values[] = {0.75f};
    XPLMInstanceSetPosition(instance, &pose, values);

    XPLMCreateWindow_t w{};
    w.structSize = sizeof(w);
    w.left = 30; w.top = 51; w.right = 60; w.bottom = 40;
    w.visible = 1;
    w.contentType = xplm_WindowContentTypePanelGraphics;
    w.drawWindowFunc = draw_window;
    window = XPLMCreateWindowEx(&w);
    XPLMSetWindowTitle(window, "panel fixture");
    XPLMCreateAvionics_t av{};
    av.structSize = sizeof(av);
    av.deviceID = "org.xphost.fixture";
    av.deviceName = "Fixture";
    av.drawOnDemand = 1;
    av.screenWidth = av.screenHeight = 16;
    av.drawCallback = draw_screen;
    av.contentType = xplm_WindowContentTypePanelGraphics;
    device = XPLMCreateAvionicsEx(&av);
    XPLMCustomizeAvionics_t c{};
    c.structSize = sizeof(c);
    c.deviceId = xplm_device_G1000_PFD_1;
    c.drawCallbackBefore = c.drawCallbackAfter = draw_builtin;
    c.contentType = xplm_WindowContentTypePanelGraphics;
    builtin = XPLMRegisterAvionicsCallbacksEx(&c);
    Check("panel registrations", window && device && builtin);
    return 1;
}
void shutdown() {
    if (!window) return;
    XPLMDestroyWindow(window);
    XPLMDestroyAvionics(device);
    XPLMUnregisterAvionicsCallbacks(builtin);
    XPLMDestroyTextureAtlas(atlas);
    XPLMDestroyFont(font);
    XPLMDestroyInstance(instance);
    window = nullptr;
}
} // namespace panel_fixture
