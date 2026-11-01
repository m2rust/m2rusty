cmake_minimum_required(VERSION 3.16)
project(neoeter_python_abi LANGUAGES C CXX)

# C ABI over the Python host for the Rust client. Shared, so the Rust side
# links one library instead of the engine's static archives and their system
# dependencies. Needs CMAKE_POSITION_INDEPENDENT_CODE (M2_BUILD_PYTHON_ABI).

add_library(neoeter_python_abi SHARED NeoeterPythonAbi.cpp)
add_library(NeoeterPython::Abi ALIAS neoeter_python_abi)

target_include_directories(neoeter_python_abi
    PUBLIC
        $<BUILD_INTERFACE:${NEOETER_PYTHON_INCLUDE_DIR}>
)

target_link_libraries(neoeter_python_abi
    PRIVATE
        neoeter_python_host
)

set_target_properties(neoeter_python_abi PROPERTIES
    CXX_VISIBILITY_PRESET hidden
    VISIBILITY_INLINES_HIDDEN ON
)

# Export only neoeter_python_*, not the engine archives linked into it.
if(UNIX AND NOT APPLE)
    target_link_options(neoeter_python_abi PRIVATE "LINKER:--exclude-libs,ALL")
endif()

install(TARGETS neoeter_python_abi LIBRARY DESTINATION lib RUNTIME DESTINATION bin)
install(FILES "${NEOETER_PYTHON_INCLUDE_DIR}/Neoeter/Python/Abi/neoeter_python.h"
    DESTINATION include/Neoeter/Python/Abi)
