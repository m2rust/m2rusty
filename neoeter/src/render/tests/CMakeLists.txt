add_executable(test_eterlib
    test_eterlib.cpp
)

target_link_libraries(test_eterlib
    PRIVATE
        neoeter_render
)

add_test(NAME EterLib_SmokeTests COMMAND test_eterlib)

foreach(TEST_NAME test_grpmath test_detector test_jpeg test_input test_ime test_shader_stats)
    add_executable(${TEST_NAME} ${TEST_NAME}.cpp)
    target_link_libraries(${TEST_NAME} PRIVATE neoeter_render)
    add_test(NAME ${TEST_NAME} COMMAND ${TEST_NAME})
endforeach()

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_eterlib test_grpmath test_detector test_jpeg test_input test_ime test_shader_stats)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()

# TA.4: headless renderer goldens.
#
# The scenes render through the real CGraphicDevice on a D3D9-compatibility
# façade. EGL's surfaceless platform + Mesa llvmpipe gives that pipeline an
# offscreen FBO without Xvfb, a window or a GPU, so `ctest` stays headless.
# The test is Linux-only: the EGL/GLX compatibility path does not exist on the
# Windows target yet.
if(UNIX)
    find_package(OpenGL QUIET COMPONENTS GL EGL)
    find_package(ZLIB QUIET)

    if(TARGET OpenGL::GL AND TARGET OpenGL::EGL AND ZLIB_FOUND)
        add_executable(test_render_golden
            render_golden.cpp
            support/OffscreenGL.cpp
            support/Png.cpp
        )

        target_link_libraries(test_render_golden
            PRIVATE
                neoeter_render
                OpenGL::GL
                OpenGL::EGL
                ZLIB::ZLIB
        )

        target_include_directories(test_render_golden PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
        target_compile_definitions(test_render_golden
            PRIVATE
                RENDER_GOLDEN_DIR="${CMAKE_CURRENT_SOURCE_DIR}/golden/render"
                RENDER_DEFAULT_OUTPUT_DIR="${CMAKE_CURRENT_BINARY_DIR}/render-output"
        )
        target_compile_options(test_render_golden
            PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)

        add_test(NAME render_golden COMMAND test_render_golden)
        set_tests_properties(render_golden PROPERTIES
            ENVIRONMENT "LIBGL_ALWAYS_SOFTWARE=1;GALLIUM_DRIVER=llvmpipe"
            TIMEOUT 180
        )
    else()
        message(WARNING
            "EGL or zlib not found; skipping the headless render goldens (TA.4)")
    endif()
endif()
