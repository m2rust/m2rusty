# C++ reference dumpers for the Rust format crates' differential tests
# (crates/m2-formats/tests/differential*.rs). One directory per format, each
# with its own CMakeLists.txt defining m2diff_<module>; scripts/differential.sh
# builds them all and runs the tests.

file(GLOB _tool_dirs LIST_DIRECTORIES true RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}" "${CMAKE_CURRENT_SOURCE_DIR}/*")
list(SORT _tool_dirs)
foreach(_dir IN LISTS _tool_dirs)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/${_dir}/CMakeLists.txt")
        add_subdirectory(${_dir})
    endif()
endforeach()
