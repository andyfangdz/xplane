// Retained drawing ABI checks use the same immediate drawing function as the
// reference, then compare both batches after changing the destination geometry.
namespace retained_fixture {
XPLMWindowID window{};
XPLMRetainedDrawing_t child{}, parent{};
void* texture{};
XPLMFontHandle font{};
int frame{};

void artwork() {
    XPLMVertex_t points[] = {{1, 3}, {5, 3}, {1, 7}};
    XPLMPolygon(0xff816243, points, 3);
    XPLMFontDrawString(font, 0xffaabbcc, 14, 2, 4, "Map", xplm_JustLeft);
    struct Vertex { float x, y, u, v; uint32_t color; };
    const Vertex vertices[] = {{1,3,0,0,0xff202122}, {5,3,1,0,0xffffffff}, {1,7,0,1,0xffffffff}};
    const uint16_t indices[] = {0,1,2};
    XPLMMesh_t mesh{3, reinterpret_cast<const float*>(vertices), 3, indices};
    XPLMDrawCall_t call{texture, {0,0,8,8}, 0, 3, 0};
    XPLMScissorPush();
    XPLMScissorIntersect(0, 8, 8, 0);
    XPLMDrawCalls(&mesh, 1, &call);
    XPLMScissorPop();
}
void draw(XPLMWindowID, void*) {
    XPLMTransformPush();
    XPLMTransformTranslate(40.f + 10.f * frame++, 70);
    XPLMTransformScale(2, -1);
    XPLMScissorPush();
    XPLMScissorIntersect(0, 7, 6, 1);
    if (!parent) {
        XPLMBeginRetainedDrawing();
        XPLMTransformPush();
        XPLMTransformTranslate(2, 1);
        XPLMDrawRetained(child);
        XPLMTransformPop();
        parent = XPLMEndRetainedDrawing();
        Check("record nested retained replay", parent != nullptr);
    }
    XPLMTransformPush();
    XPLMTransformTranslate(2, 1);
    artwork();
    XPLMTransformPop();
    if (std::getenv("XPHOST_FIXTURE_BAD_RETAINED")) {
        XPLMTransformPush();
        XPLMTransformRotate(0, 0, 0);
        XPLMDrawRetained(parent);
        XPLMTransformPop();
        XPLMDestroyTexture(texture);
        texture = nullptr;
    }
    XPLMDrawRetained(parent);
    XPLMScissorPop();
    XPLMTransformPop();
}
int open(XPLMCommandRef, XPLMCommandPhase phase, void*) {
    if (phase != xplm_CommandBegin || window) return 1;
    const unsigned char pixel[] = {17, 43, 59, 255};
    texture = XPLMCreateTexture(pixel, 1, 1);
    font = XPLMCreateFont(xplm_CharSetUnicode);
    Check("retained fixture font", XPLMFontAddFace(font, "Resources/fonts/fixture.ttf") == 1);
    XPLMBeginRetainedDrawing();
    // A getter may re-enter the plugin during recording without ending it.
    XPLMGetDataf(XPLMFindDataRef("xphost/test/value"));
    artwork();
    child = XPLMEndRetainedDrawing();
    Check("record outside drawing callback", child != nullptr);
    XPLMCreateWindow_t w{};
    w.structSize = sizeof(w);
    w.top = 100; w.right = 200; w.visible = 1;
    w.contentType = xplm_WindowContentTypePanelGraphics;
    w.drawWindowFunc = draw;
    window = XPLMCreateWindowEx(&w);
    if (std::getenv("XPHOST_FIXTURE_BAD_RETAINED")) {
        XPLMBeginRetainedDrawing();
        XPLMBeginRetainedDrawing();
        Check("nested begin rejected", !XPLMEndRetainedDrawing());
        XPLMBeginRetainedDrawing();  // Must be discarded at command return.
    }
    return 1;
}
void shutdown() {
    if (!window) return;
    XPLMDestroyWindow(window);
    if (parent) XPLMDestroyRetainedDrawing(parent);
    XPLMDestroyRetainedDrawing(child);
    if (texture) XPLMDestroyTexture(texture);
    XPLMDestroyFont(font);
    window = nullptr;
    child = parent = nullptr;
    frame = 0;
}
int close(XPLMCommandRef, XPLMCommandPhase phase, void*) {
    if (phase == xplm_CommandBegin) shutdown();
    return 1;
}
} // namespace retained_fixture
