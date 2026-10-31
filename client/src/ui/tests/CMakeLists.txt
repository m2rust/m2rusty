add_executable(test_userinterface
    test_userinterface.cpp
)

add_executable(test_python_client_api
    test_python_client_api.cpp
)

target_link_libraries(test_userinterface
    PRIVATE
        client_ui
        pthread
        dl
)

target_link_libraries(test_python_client_api
    PRIVATE
        client_ui
        pthread
        dl
)

target_include_directories(test_userinterface
    PRIVATE
        ${CLIENT_INCLUDE_DIR}
        ${CLIENT_INCLUDE_DIR}/client_ui
        ${PYTHON_INCLUDE_DIRS}
)

if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../../../extern/include")
    target_include_directories(test_userinterface PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../../../extern/include)
    target_include_directories(test_python_client_api PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../../../extern/include)
endif()

if(UNIX)
    target_compile_definitions(test_userinterface PRIVATE OPENMT2_PLATFORM_LINUX=1 OPENMT2_RENDERER_GL=1 OPENMT2_SDL_WINDOW=1 _GNU_SOURCE)
    target_compile_definitions(test_python_client_api PRIVATE OPENMT2_PLATFORM_LINUX=1 OPENMT2_RENDERER_GL=1 OPENMT2_SDL_WINDOW=1 _GNU_SOURCE)
endif()

add_test(NAME UserInterface_SmokeTests COMMAND test_userinterface)
add_test(NAME test_python_client_api
    COMMAND test_python_client_api
        ${CMAKE_CURRENT_SOURCE_DIR}/data/python_api_client.txt
)

add_executable(test_packet test_packet.cpp)
target_link_libraries(test_packet PRIVATE client_ui pthread dl)
target_include_directories(test_packet
    PRIVATE
        ${CLIENT_INCLUDE_DIR}
        ${CLIENT_INCLUDE_DIR}/client_ui
        ${PYTHON_INCLUDE_DIRS}
)
if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../../../extern/include")
    target_include_directories(test_packet PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../../../extern/include)
endif()
if(UNIX)
    target_compile_definitions(test_packet PRIVATE OPENMT2_PLATFORM_LINUX=1 OPENMT2_RENDERER_GL=1 OPENMT2_SDL_WINDOW=1 _GNU_SOURCE)
endif()
add_test(NAME test_packet COMMAND test_packet)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_userinterface test_packet test_python_client_api)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()

add_executable(test_mark_image test_mark_image.cpp)
target_link_libraries(test_mark_image PRIVATE client_ui pthread dl)
target_include_directories(test_mark_image
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
        ${CLIENT_INCLUDE_DIR}
        ${CLIENT_INCLUDE_DIR}/client_ui
        ${PYTHON_INCLUDE_DIRS}
)
if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../../../extern/include")
    target_include_directories(test_mark_image PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../../../extern/include)
endif()
if(UNIX)
    target_compile_definitions(test_mark_image PRIVATE OPENMT2_PLATFORM_LINUX=1 OPENMT2_RENDERER_GL=1 OPENMT2_SDL_WINDOW=1 _GNU_SOURCE)
endif()
add_test(NAME test_mark_image COMMAND test_mark_image)
