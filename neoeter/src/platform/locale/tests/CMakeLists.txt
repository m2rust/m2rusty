# src/neoeter_platform_locale/tests/CMakeLists.txt

add_executable(test_locale test_locale.cpp)
add_executable(test_locale_codecs test_locale_codecs.cpp)
target_link_libraries(test_locale PRIVATE neoeter_platform_locale)
target_link_libraries(test_locale_codecs PRIVATE neoeter_platform_locale)
add_test(NAME test_locale COMMAND test_locale)
add_test(NAME test_locale_codecs COMMAND test_locale_codecs)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_locale test_locale_codecs)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()
