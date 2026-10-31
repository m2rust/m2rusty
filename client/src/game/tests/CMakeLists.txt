add_executable(test_gamelib
    test_gamelib.cpp
)

target_link_libraries(test_gamelib
    PRIVATE
        client_game
        neoeter_effect
        neoeter_terrain
        neoeter_model_speedtree
        neoeter_model_grn
        neoeter_audio
        neoeter_render
        neoeter_io
        neoeter_image
        neoeter_platform_math
        neoeter_platform_locale
        neoeter_platform_base
        pthread
        dl
)

target_include_directories(test_gamelib
    PRIVATE
        ${CLIENT_INCLUDE_DIR}
        ${CLIENT_INCLUDE_DIR}/client_game
)

if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../../../extern/include")
    target_include_directories(test_gamelib PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../../../extern/include)
endif()

if(UNIX)
    target_compile_definitions(test_gamelib PRIVATE OPENMT2_PLATFORM_LINUX=1 OPENMT2_RENDERER_GL=1 OPENMT2_SDL_WINDOW=1 _GNU_SOURCE)
endif()

add_test(NAME GameLib_SmokeTests COMMAND test_gamelib)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_gamelib)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()
