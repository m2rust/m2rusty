# src/neoeter_platform_base/tests/CMakeLists.txt

set(TEST_TARGETS
    test_tea
    test_crc32
    test_lzo
    test_file
    test_stl
    test_timer
    test_poly
    test_crc32_vectors
    test_lzo_invariants
    test_tea_invariants
    test_image_loader
    test_render_seam
)

foreach(TEST_NAME ${TEST_TARGETS})
    add_executable(${TEST_NAME} ${TEST_NAME}.cpp)
    target_link_libraries(${TEST_NAME} PRIVATE neoeter_platform_base)
    target_include_directories(${TEST_NAME} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    # Asserts must stay active in every build type (D15).
    target_compile_options(${TEST_NAME} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
    add_test(NAME ${TEST_NAME} COMMAND ${TEST_NAME})
endforeach()
