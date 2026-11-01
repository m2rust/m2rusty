add_executable(m2diff_ui UiDump.cpp)
target_link_libraries(m2diff_ui
    PRIVATE
        neoeter_python_bindings
        neoeter_python_host
        neoeter_render
        neoeter_model_grn
        neoeter_effect
        neoeter_io
        neoeter_platform_base
        ${PYTHON_LIBRARIES}
        util
        dl
)
# The dumper reads and drives private state of the real window manager, the
# text instance and the IME (hit-test captures, glyph vectors, edit buffer):
# the point is to run the real code, not a copy of it.
target_compile_options(m2diff_ui PRIVATE -fno-access-control)
