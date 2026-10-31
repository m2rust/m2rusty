#include <Client/Game/StdAfx.h>
#include <Client/Game/GameUtil.h>
#include <Client/Game/ItemManager.h>
#include <Client/Game/RaceManager.h>
#include <Client/Game/PropertyManager.h>
#include <Client/Game/ActorInstance.h>
#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstring>

namespace
{

void test_game_math_and_util()
{
    // Rotation from position (Metin2 uses (0, -1, 0) as standard 0-degree vector)
    float degSouth = GetDegreeFromPosition(0.0f, -10.0f);
    assert(std::fabs(degSouth - 0.0f) < 1.0f || std::fabs(degSouth - 360.0f) < 1.0f);

    float degEast = GetDegreeFromPosition(10.0f, 0.0f);
    assert(std::fabs(degEast - 90.0f) < 1.0f);

    float degNorth = GetDegreeFromPosition(0.0f, 10.0f);
    assert(std::fabs(degNorth - 180.0f) < 1.0f);

    float degWest = GetDegreeFromPosition(-10.0f, 0.0f);
    assert(std::fabs(degWest - 270.0f) < 1.0f);

    // Rotating direction
    int dirRight = GetRotatingDirection(80.0f, 10.0f);
    assert(dirRight == DEGREE_DIRECTION_RIGHT);

    int dirLeft = GetRotatingDirection(10.0f, 80.0f);
    assert(dirLeft == DEGREE_DIRECTION_LEFT);

    std::printf("[PASS] test_game_math_and_util\n");
}

void test_item_manager()
{
    static_assert(sizeof(CItemData::TItemTable) == 156, "TItemTable stride must match item_proto stride (156 bytes)");
    static_assert(sizeof(CItemData::TItemLimit) == 5, "TItemLimit size must be 5 bytes");
    static_assert(sizeof(CItemData::TItemApply) == 5, "TItemApply size must be 5 bytes");

    CItemManager itemMgr;

    CItemData* pItem1 = itemMgr.MakeItemData(1001);
    assert(pItem1 != nullptr);

    CItemData* pFound = nullptr;
    BOOL foundOk = itemMgr.GetItemDataPointer(1001, &pFound);
    assert(foundOk && pFound == pItem1);

    // Make another item
    CItemData* pItem2 = itemMgr.MakeItemData(2002);
    assert(pItem2 != nullptr);
    assert(pItem2 != pItem1);

    // Select item
    BOOL selectOk = itemMgr.SelectItemData(2002);
    assert(selectOk);
    assert(itemMgr.GetSelectedItemDataPointer() == pItem2);

    // Non-existent item
    CItemData* pMissing = nullptr;
    assert(!itemMgr.GetItemDataPointer(99999, &pMissing));

    std::printf("[PASS] test_item_manager\n");
}

void test_race_manager()
{
    CRaceManager raceMgr;

    raceMgr.RegisterRaceName(1, "warrior_m");
    raceMgr.RegisterRaceName(2, "assassin_w");

    raceMgr.CreateRace(1);
    raceMgr.CreateRace(2);

    CRaceData* pRaceData1 = nullptr;
    BOOL ok = raceMgr.GetRaceDataPointer(1, &pRaceData1);
    assert(ok && pRaceData1 != nullptr);

    raceMgr.SelectRace(2);
    assert(raceMgr.GetSelectedRaceDataPointer() != nullptr);

    std::printf("[PASS] test_race_manager\n");
}

void test_property_manager()
{
    CPropertyManager propMgr;

    // Verify singleton creation and state
    assert(CPropertyManager::InstancePtr() != nullptr);

    std::printf("[PASS] test_property_manager\n");
}

void test_actor_instance_basics()
{
    CActorInstance actor;

    actor.SetPosition(100.0f, 200.0f, 50.0f);
    const D3DXVECTOR3& pos = actor.GetPosition();
    assert(std::fabs(pos.x - 100.0f) < 1e-4f);
    assert(std::fabs(pos.y - 200.0f) < 1e-4f);
    assert(std::fabs(pos.z - 50.0f) < 1e-4f);

    actor.SetRotation(135.0f);
    assert(std::fabs(actor.GetRotation() - 135.0f) < 1e-4f);

    std::printf("[PASS] test_actor_instance_basics\n");
}

} // namespace

int main()
{
    test_game_math_and_util();
    test_item_manager();
    test_race_manager();
    test_property_manager();
    test_actor_instance_basics();

    std::printf("All GameLib smoke tests passed successfully!\n");
    return 0;
}
