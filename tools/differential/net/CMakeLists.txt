# Layout, header-map, sequence-table and framing dumper for crates/m2-net.
# Packet.h is header-only for this purpose: take client_ui's include paths and
# definitions without linking (and so without building) client_ui.
add_executable(m2diff_net NetDump.cpp)
target_include_directories(m2diff_net PRIVATE
    $<TARGET_PROPERTY:client_ui,INTERFACE_INCLUDE_DIRECTORIES>
    ${PYTHON_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../client/src/ui)
target_compile_definitions(m2diff_net PRIVATE
    $<TARGET_PROPERTY:client_ui,INTERFACE_COMPILE_DEFINITIONS>)
target_link_libraries(m2diff_net PRIVATE
    client_game
    neoeter_render
    neoeter_io
    neoeter_platform_math
    neoeter_platform_locale
    neoeter_platform_base
    pthread
    dl)
