# src/neoeter_image/tests/CMakeLists.txt

set(TEST_TARGETS
    test_image
    test_dxt_invariants
)

foreach(TEST_NAME ${TEST_TARGETS})
    add_executable(${TEST_NAME} ${TEST_NAME}.cpp)
    target_link_libraries(${TEST_NAME} PRIVATE neoeter_image)
    add_test(NAME ${TEST_NAME} COMMAND ${TEST_NAME})

    # TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
    target_include_directories(${TEST_NAME} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${TEST_NAME} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()
