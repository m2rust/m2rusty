add_executable(test_scriptlib test_scriptlib.cpp)
add_executable(test_python_scriptlib_api test_python_scriptlib_api.cpp)

target_link_libraries(test_scriptlib
    PRIVATE
        neoeter_python_host
)

target_link_libraries(test_python_scriptlib_api
    PRIVATE
        neoeter_python_host
)

add_test(NAME ScriptLib_SmokeTests COMMAND test_scriptlib)
add_test(NAME test_python_scriptlib_api
    COMMAND test_python_scriptlib_api
        ${CMAKE_CURRENT_SOURCE_DIR}/data/python_api_scriptlib.txt
)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_scriptlib test_python_scriptlib_api)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()
