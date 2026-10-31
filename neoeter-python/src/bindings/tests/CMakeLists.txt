add_executable(test_eterpythonlib
    test_eterpythonlib.cpp
)

add_executable(test_python_eterpythonlib_api
    test_python_eterpythonlib_api.cpp
)

target_link_libraries(test_eterpythonlib
    PRIVATE
        neoeter_python_bindings
        neoeter_python_host
        neoeter_render
        neoeter_model_grn
        neoeter_effect
        neoeter_io
        neoeter_platform_base
        ${PYTHON_LIBRARIES}
        util
        dl
)

target_link_libraries(test_python_eterpythonlib_api
    PRIVATE
        neoeter_python_bindings
        ${PYTHON_LIBRARIES}
        util
        dl
)

target_include_directories(test_eterpythonlib
    PRIVATE
        ${NEOETER_PYTHON_INCLUDE_DIR}
        ${NEOETER_PYTHON_INCLUDE_DIR}/neoeter_python_bindings
        ${NEOETER_PYTHON_INCLUDE_DIR}/python2.7
)

if(UNIX)
    target_compile_definitions(test_eterpythonlib PRIVATE OPENMT2_PLATFORM_LINUX=1 OPENMT2_RENDERER_GL=1 OPENMT2_SDL_WINDOW=1 _GNU_SOURCE)
    target_compile_options(test_eterpythonlib PRIVATE -Wno-register)
    if(CMAKE_CXX_COMPILER_ID MATCHES "Clang")
        target_compile_options(test_eterpythonlib PRIVATE -Wno-macro-redefined)
    endif()
endif()

add_test(NAME EterPythonLib_SmokeTests COMMAND test_eterpythonlib)
add_test(NAME test_python_eterpythonlib_api
    COMMAND test_python_eterpythonlib_api
        ${CMAKE_CURRENT_SOURCE_DIR}/data/python_api_eterpythonlib.txt
)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_eterpythonlib test_python_eterpythonlib_api)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()
