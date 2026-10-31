# src/neoeter_platform_math/tests/CMakeLists.txt

add_executable(test_sphere test_sphere.cpp)
add_executable(test_sphere_invariants test_sphere_invariants.cpp)
target_link_libraries(test_sphere PRIVATE neoeter_platform_math)
target_link_libraries(test_sphere_invariants PRIVATE neoeter_platform_math)
add_test(NAME test_sphere COMMAND test_sphere)
add_test(NAME test_sphere_invariants COMMAND test_sphere_invariants)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_sphere test_sphere_invariants)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()
