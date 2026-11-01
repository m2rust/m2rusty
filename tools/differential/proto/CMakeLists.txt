add_executable(m2diff_proto ProtoDump.cpp)
# The real CItemManager (client_game), CPythonNonPlayer and CPythonSkill
# (client_ui).
target_link_libraries(m2diff_proto PRIVATE client_ui pthread dl)
