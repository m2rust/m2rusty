// Backend selection (T1.7, invariant R8).
//
// The backend is chosen at configure time: one binary per backend, no runtime
// #if maze. Only the GL backend exists today; OPENMT2_RENDERER_GL is defined by
// the build for that configuration.

#include <Platform/RenderBackend.h>

#if defined(OPENMT2_RENDERER_GL)

#include "backends/gl/GLRenderBackend.h"

IRenderBackend& Platform_GetRenderBackend()
{
    // GL state is process-wide, so the backend is too. Construction performs no
    // GL call, which keeps headless tests that never draw independent of a
    // context.
    static eterbase::gl::GLRenderBackend backend;
    return backend;
}

#endif // OPENMT2_RENDERER_GL
