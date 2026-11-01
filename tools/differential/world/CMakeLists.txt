add_executable(m2diff_world WorldDump.cpp)
target_link_libraries(m2diff_world PRIVATE
    client_game neoeter_effect neoeter_terrain neoeter_model_speedtree neoeter_model_grn neoeter_audio
    neoeter_render neoeter_io neoeter_image neoeter_platform_math neoeter_platform_locale neoeter_platform_base
    pthread dl)
target_link_options(m2diff_world PRIVATE -Wl,--allow-multiple-definition)
