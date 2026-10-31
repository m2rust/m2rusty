# Role-level namespaces for the consolidated engine.
#
# Every package defines its own namespaced target (Neoeter::Platform::Base,
# Neoeter::IO, ...). This file adds the grouping target for the multi-package
# role only; Neoeter::Platform links base + locale + math.

function(_neoeter_role _name)
    if(TARGET Neoeter::${_name})
        return()
    endif()
    add_library(Neoeter_${_name} INTERFACE)
    add_library(Neoeter::${_name} ALIAS Neoeter_${_name})
    foreach(_dep IN LISTS ARGN)
        if(TARGET ${_dep})
            target_link_libraries(Neoeter_${_name} INTERFACE ${_dep})
        endif()
    endforeach()
endfunction()

_neoeter_role(Platform neoeter_platform_base neoeter_platform_locale neoeter_platform_math)
