# src/client_webbrowser/tests/CMakeLists.txt

add_executable(test_webbrowser test_webbrowser.cpp)
target_link_libraries(test_webbrowser PRIVATE client_webbrowser)
add_test(NAME test_webbrowser COMMAND test_webbrowser)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_webbrowser)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()
