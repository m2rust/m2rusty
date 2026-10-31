cmake_minimum_required(VERSION 3.16)
project(client_game LANGUAGES C CXX)

if(CMAKE_SOURCE_DIR STREQUAL PROJECT_SOURCE_DIR)
    set(GAMELIB_STANDALONE ON)
    set(CMAKE_CXX_STANDARD 17)
    set(CMAKE_CXX_STANDARD_REQUIRED ON)
    set(CMAKE_CXX_EXTENSIONS OFF)
    option(BUILD_TESTING "Build client_game tests" ON)
else()
    set(GAMELIB_STANDALONE OFF)
endif()

if(GAMELIB_STANDALONE)
    include(FetchContent)

    foreach(dep EterBase:pkg_eterbase EterLocale:pkg_eterlocale EterPack:pkg_eterpack EterImageLib:pkg_eterimagelib SphereLib:pkg_spherelib client_webbrowser:pkg_cwebbrowser EterLib:pkg_eterlib MilesLib:pkg_mileslib EterGrnLib:pkg_etergrnlib SpeedTreeLib:pkg_speedtreelib PRTerrainLib:pkg_prterrainlib EffectLib:pkg_effectlib)
        string(REPLACE ":" ";" dep_list ${dep})
        list(GET dep_list 0 dep_name)
        list(GET dep_list 1 dep_repo)
        if(NOT TARGET ${dep_name})
            if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../${dep_name}/CMakeLists.txt")
                set(BUILD_TESTING_SAVED ${BUILD_TESTING})
                set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
                add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../${dep_name}" "${CMAKE_BINARY_DIR}/deps/${dep_name}")
                set(BUILD_TESTING ${BUILD_TESTING_SAVED} CACHE BOOL "" FORCE)
            else()
                FetchContent_Declare(
                    ${dep_name}
                    GIT_REPOSITORY https://github.com/open-mt2/${dep_repo}.git
                    GIT_TAG main
                )
                set(BUILD_TESTING_SAVED ${BUILD_TESTING})
                set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
                FetchContent_MakeAvailable(${dep_name})
                set(BUILD_TESTING ${BUILD_TESTING_SAVED} CACHE BOOL "" FORCE)
            endif()
        endif()
    endforeach()
endif()

file(GLOB_RECURSE GAMELIB_SOURCES CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/*.cpp"
)
list(FILTER GAMELIB_SOURCES EXCLUDE REGEX "/tests/")

add_library(client_game STATIC ${GAMELIB_SOURCES})
add_library(Client::Game ALIAS client_game)

target_include_directories(client_game
    PUBLIC
        $<BUILD_INTERFACE:${CLIENT_INCLUDE_DIR}>
        $<BUILD_INTERFACE:${CLIENT_INCLUDE_DIR}>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
)

if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../../extern/include")
    target_include_directories(client_game PUBLIC $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/../../extern/include>)
endif()

target_link_libraries(client_game
    PUBLIC
        neoeter_platform_base
        neoeter_io
        neoeter_render
        neoeter_audio
        neoeter_model_grn
        neoeter_model_speedtree
        neoeter_terrain
        neoeter_effect
)

if(UNIX)
    target_compile_definitions(client_game PUBLIC
        OPENMT2_PLATFORM_LINUX=1
        OPENMT2_RENDERER_GL=1
        OPENMT2_SDL_WINDOW=1
        _GNU_SOURCE
    )
endif()

if((GAMELIB_STANDALONE AND BUILD_TESTING) OR (NOT GAMELIB_STANDALONE AND BUILD_TESTS))
    enable_testing()
    add_subdirectory(tests)
endif()
