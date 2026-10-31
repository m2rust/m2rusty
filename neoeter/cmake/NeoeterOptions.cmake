# Neoeter build options.
#
# One preset file, two targets: these options are shared by the Linux build and
# the MinGW-w64 cross build (D7/D8). Do not add platform-specific forks here.

set(CMAKE_CXX_STANDARD 17 CACHE STRING "C++ standard")
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

option(NEOETER_BUILD_TESTS "Build Neoeter tests" ON)
option(NEOETER_STANDALONE "Build Neoeter on its own (not as part of the devkit)" OFF)

if(NEOETER_STANDALONE)
    include(FetchContent)
endif()
