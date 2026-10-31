#include <Client/UI/StdAfx.h>
#include <Client/UI/AffectFlagContainer.h>
#include <Client/UI/InsultChecker.h>
#include <Client/UI/PythonSystem.h>
#include <Client/UI/Packet.h>
#include <Client/UI/GameType.h>
#include <Neoeter/Audio/SoundManager.h>
#include <cassert>
#include <cstdio>
#include <cstring>

namespace
{

void test_affect_flag_container()
{
    CAffectFlagContainer container;
    assert(!container.IsSet(0));
    assert(!container.IsSet(32));
    assert(!container.IsSet(63));

    container.Set(5, true);
    assert(container.IsSet(5));
    assert(!container.IsSet(4));

    container.Set(42, true);
    assert(container.IsSet(42));

    CAffectFlagContainer copy;
    copy.CopyInstance(container);
    assert(copy.IsSet(5));
    assert(copy.IsSet(42));
    assert(!copy.IsSet(1));

    container.Clear();
    assert(!container.IsSet(5));
    assert(!container.IsSet(42));

    std::printf("[PASS] test_affect_flag_container\n");
}

void test_insult_checker()
{
    CInsultChecker checker;
    checker.AppendInsult("badword");
    checker.AppendInsult("spam");

    const char* text1 = "this is badword here";
    assert(checker.IsInsultIn(text1, (UINT)std::strlen(text1)));

    const char* text2 = "clean sentence here";
    assert(!checker.IsInsultIn(text2, (UINT)std::strlen(text2)));

    char filterBuf[64];
    std::strcpy(filterBuf, "hello badword world");
    checker.FilterInsult(filterBuf, (UINT)std::strlen(filterBuf));
    assert(std::strstr(filterBuf, "badword") == nullptr);
    assert(std::strstr(filterBuf, "*") != nullptr);

    std::printf("[PASS] test_insult_checker\n");
}

void test_python_system_config()
{
    CSoundManager soundManager;
    CPythonSystem system;
    CPythonSystem::TConfig config;
    config.bWindowed = true;
    config.width = 1024;
    config.height = 768;
    config.bpp = 32;
    config.frequency = 60;
    config.music_volume = 0.5f;
    config.voice_volume = 1;
    config.gamma = 3;
    config.is_software_cursor = false;
    config.is_object_culling = true;
    config.bDecompressDDS = false;

    system.SetConfig(&config);
    const CPythonSystem::TConfig* pConfig = system.GetConfig();
    assert(pConfig != nullptr);
    assert(pConfig->bWindowed == true);
    assert(pConfig->width == 1024);
    assert(pConfig->height == 768);
    assert(pConfig->gamma == 3);

    std::printf("[PASS] test_python_system_config\n");
}

void test_show_cursor_counter()
{
    // The client hides the cursor with a Windows-style do/while loop:
    //     do { i = ShowCursor(FALSE); } while (i >= 0);
    // A platform stub that always returns 0 makes that loop infinite, which
    // used to hang the logo-close path before the login window could open.
    int guard = 0;
    int count = 0;
    do {
        count = ShowCursor(FALSE);
    } while (count >= 0 && ++guard < 16);
    assert(count < 0);

    guard = 0;
    do {
        count = ShowCursor(TRUE);
    } while (count < 0 && ++guard < 16);
    assert(count >= 0);

    assert(ShowCursor(FALSE) < 0);
    assert(ShowCursor(FALSE) < 0);
    assert(ShowCursor(TRUE) >= 0);

    std::printf("[PASS] test_show_cursor_counter\n");
}

void test_packet_structure_alignment()
{
    static_assert(sizeof(LONG) == 4, "LONG must be 4 bytes");
    static_assert(sizeof(TItemData) == 4 + 1 + 4 + 4 + 4 * ITEM_SOCKET_SLOT_MAX_NUM + sizeof(TPlayerItemAttribute) * ITEM_ATTRIBUTE_SLOT_MAX_NUM, "TItemData size check");
    static_assert(sizeof(TShopItemData) == 4 + 4 + 1 + 1 + 4 * ITEM_SOCKET_SLOT_MAX_NUM + sizeof(TPlayerItemAttribute) * ITEM_ATTRIBUTE_SLOT_MAX_NUM, "TShopItemData size check");

    // Verify packet header structures and alignment consistency
    assert(sizeof(TPacketHeader) == 1);
    assert(sizeof(TPacketCGAttack) > 0);
    assert(sizeof(TPacketGCHandshake) > 0);
    assert(sizeof(TPacketGCPhase) == 2); // header(1) + phase(1)
    assert(sizeof(TPacketGCCharacterAdd) > 0);
    assert(sizeof(TPacketGCMove) > 0);

    std::printf("[PASS] test_packet_structure_alignment\n");
}

} // anonymous namespace

int main()
{
    std::printf("Running UserInterface unit and smoke tests...\n");
    test_affect_flag_container();
    test_insult_checker();
    test_python_system_config();
    test_show_cursor_counter();
    test_packet_structure_alignment();
    std::printf("All UserInterface smoke tests passed successfully!\n");
    return 0;
}
