#pragma once

#include <Client/Game/RaceData.h>
#include <Client/Game/ActorInstance.h>

#include <Client/UI/AffectFlagContainer.h>

class CInstanceBase
{
  public:
    struct SCreateData
    {
        BYTE m_bType = 0U;
        DWORD m_dwStateFlags = 0U;
        DWORD m_dwEmpireID = 0U;
        DWORD m_dwGuildID = 0U;
        DWORD m_dwLevel = 0U;
        DWORD m_dwVID = 0U;
        DWORD m_dwRace = 0U;
        DWORD m_dwMovSpd = 0U;
        DWORD m_dwAtkSpd = 0U;
        LONG m_lPosX = 0;
        LONG m_lPosY = 0;
        FLOAT m_fRot = 0.0F;
        DWORD m_dwArmor = 0U;
        DWORD m_dwWeapon = 0U;
        DWORD m_dwHair = 0U;
        DWORD m_dwMountVnum = 0U;

        short m_sAlignment = 0;
        BYTE m_byPKMode = 0U;
        CAffectFlagContainer m_kAffectFlags;

        std::string m_stName;

        bool m_isMain = false;
    };

    using TType = DWORD;

    enum EDirection
    {
        DIR_NORTH,
        DIR_NORTHEAST,
        DIR_EAST,
        DIR_SOUTHEAST,
        DIR_SOUTH,
        DIR_SOUTHWEST,
        DIR_WEST,
        DIR_NORTHWEST,
        DIR_MAX_NUM,
    };

    enum
    {
        FUNC_WAIT = 0,
        FUNC_MOVE = 1,
        FUNC_ATTACK = 2,
        FUNC_COMBO = 3,
        FUNC_MOB_SKILL = 4,
        FUNC_EMOTION = 5,
        FUNC_SKILL = 0x80,
    };

    enum
    {
        AFFECT_YMIR = 0,
        AFFECT_INVISIBILITY = 1,
        AFFECT_SPAWN = 2,

        AFFECT_POISON = 3,
        AFFECT_SLOW = 4,
        AFFECT_STUN = 5,

        AFFECT_DUNGEON_READY = 6, // 던전에서 준비 상태
        AFFECT_SHOW_ALWAYS = 7,   // AFFECT_DUNGEON_UNIQUE 에서 변경(클라이언트에서
                                  // 컬링되지않음)

        AFFECT_BUILDING_CONSTRUCTION_SMALL = 8,
        AFFECT_BUILDING_CONSTRUCTION_LARGE = 9,
        AFFECT_BUILDING_UPGRADE = 10,

        AFFECT_MOV_SPEED_POTION = 11, // 11
        AFFECT_ATT_SPEED_POTION = 12, // 12

        AFFECT_FISH_MIND = 13, // 13

        AFFECT_JEONGWI = 14,             // 14 전귀혼
        AFFECT_GEOMGYEONG = 15,          // 15 검경
        AFFECT_CHEONGEUN = 16,           // 16 천근추
        AFFECT_GYEONGGONG = 17,          // 17 경공술
        AFFECT_EUNHYEONG = 18,           // 18 은형법
        AFFECT_GWIGEOM = 19,             // 19 귀검
        AFFECT_GONGPO = 20,              // 20 공포
        AFFECT_JUMAGAP = 21,             // 21 주마갑
        AFFECT_HOSIN = 22,               // 22 호신
        AFFECT_BOHO = 23,                // 23 보호
        AFFECT_KWAESOK = 24,             // 24 쾌속
        AFFECT_HEUKSIN = 25,             // 25 흑신수호
        AFFECT_MUYEONG = 26,             // 26 무영진
        AFFECT_REVIVE_INVISIBILITY = 27, // 27 부활 무적
        AFFECT_FIRE = 28,                // 28 지속 불
        AFFECT_GICHEON = 29,             // 29 기천 대공
        AFFECT_JEUNGRYEOK = 30,          // 30 증력술
        AFFECT_DASH = 31,                // 31 대쉬
        AFFECT_PABEOP = 32,              // 32 파법술
        AFFECT_FALLEN_CHEONGEUN = 33,    // 33 다운 그레이드 천근추
        AFFECT_POLYMORPH = 34,           // 34 폴리모프
        AFFECT_WAR_FLAG1 = 35,           // 35
        AFFECT_WAR_FLAG2 = 36,           // 36
        AFFECT_WAR_FLAG3 = 37,           // 37
        AFFECT_CHINA_FIREWORK = 38,      // 38
        AFFECT_PREMIUM_SILVER = 39,
        AFFECT_PREMIUM_GOLD = 40,
        AFFECT_RAMADAN_RING = 41, // 41 초승달 반지 착용 Affect

        AFFECT_NUM = 64,

        AFFECT_HWAYEOM = AFFECT_GEOMGYEONG,
    };

    enum
    {
        NEW_AFFECT_MOV_SPEED = 200,
        NEW_AFFECT_ATT_SPEED = 201,
        NEW_AFFECT_ATT_GRADE = 202,
        NEW_AFFECT_INVISIBILITY = 203,
        NEW_AFFECT_STR = 204,
        NEW_AFFECT_DEX = 205, // 205
        NEW_AFFECT_CON = 206,
        NEW_AFFECT_INT = 207,
        NEW_AFFECT_FISH_MIND_PILL = 208,

        NEW_AFFECT_POISON = 209,
        NEW_AFFECT_STUN = 210, // 210
        NEW_AFFECT_SLOW = 211,
        NEW_AFFECT_DUNGEON_READY = 212,
        NEW_AFFECT_DUNGEON_UNIQUE = 213,

        NEW_AFFECT_BUILDING = 214,
        NEW_AFFECT_REVIVE_INVISIBLE = 215, // 215
        NEW_AFFECT_FIRE = 216,
        NEW_AFFECT_CAST_SPEED = 217,
        NEW_AFFECT_HP_RECOVER_CONTINUE = 218,
        NEW_AFFECT_SP_RECOVER_CONTINUE = 219,

        NEW_AFFECT_POLYMORPH = 220, // 220
        NEW_AFFECT_MOUNT = 221,

        NEW_AFFECT_WAR_FLAG = 222, // 222

        NEW_AFFECT_BLOCK_CHAT = 223, // 223
        NEW_AFFECT_CHINA_FIREWORK = 224,

        NEW_AFFECT_BOW_DISTANCE = 225, // 225

        NEW_AFFECT_EXP_BONUS = 500,     // 경험의 반지
        NEW_AFFECT_ITEM_BONUS = 501,    // 도둑의 장갑
        NEW_AFFECT_SAFEBOX = 502,       // PREMIUM_SAFEBOX,
        NEW_AFFECT_AUTOLOOT = 503,      // PREMIUM_AUTOLOOT,
        NEW_AFFECT_FISH_MIND = 504,     // PREMIUM_FISH_MIND,
        NEW_AFFECT_MARRIAGE_FAST = 505, // 원앙의 깃털 (금슬),
        NEW_AFFECT_GOLD_BONUS = 506,

        NEW_AFFECT_MALL = 510,                // 몰 아이템 에펙트
        NEW_AFFECT_NO_DEATH_PENALTY = 511,    // 용신의 가호 (경험치 패널티를 한번 막아준다)
        NEW_AFFECT_SKILL_BOOK_BONUS = 512,    // 선인의 교훈 (책 수련 성공 확률이 50% 증가)
        NEW_AFFECT_SKILL_BOOK_NO_DELAY = 513, // 주안 술서 (책 수련 딜레이 없음)

        NEW_AFFECT_EXP_BONUS_EURO_FREE = 516, // 경험의 반지 (유럽 버전 14 레벨 이하 기본 효과)
        NEW_AFFECT_EXP_BONUS_EURO_FREE_UNDER_15 = 517,

        NEW_AFFECT_AUTO_HP_RECOVERY = 534, // 자동물약 HP
        NEW_AFFECT_AUTO_SP_RECOVERY = 535, // 자동물약 SP

        NEW_AFFECT_DRAGON_SOUL_QUALIFIED = 540,
        NEW_AFFECT_DRAGON_SOUL_DECK1 = 541,
        NEW_AFFECT_DRAGON_SOUL_DECK2 = 542,

        NEW_AFFECT_RAMADAN_ABILITY = 300,
        NEW_AFFECT_RAMADAN_RING = 301, // 라마단 이벤트용 특수아이템 초승달의 반지 착용 유무

        NEW_AFFECT_NOG_POCKET_ABILITY = 302,

        NEW_AFFECT_QUEST_START_IDX = 1000,
    };

    enum
    {
        STONE_SMOKE1 = 0, // 99%
        STONE_SMOKE2 = 1, // 85%
        STONE_SMOKE3 = 2, // 80%
        STONE_SMOKE4 = 3, // 60%
        STONE_SMOKE5 = 4, // 45%
        STONE_SMOKE6 = 5, // 40%
        STONE_SMOKE7 = 6, // 20%
        STONE_SMOKE8 = 7, // 10%
        STONE_SMOKE_NUM = 4,
    };

    enum EBuildingAffect
    {
        BUILDING_CONSTRUCTION_SMALL = 0,
        BUILDING_CONSTRUCTION_LARGE = 1,
        BUILDING_UPGRADE = 2,
    };

    enum
    {
        WEAPON_DUALHAND,
        WEAPON_ONEHAND,
        WEAPON_TWOHAND,
        WEAPON_NUM,
    };

    enum
    {
        EMPIRE_NONE,
        EMPIRE_A,
        EMPIRE_B,
        EMPIRE_C,
        EMPIRE_NUM,
    };

    enum
    {
        NAMECOLOR_MOB = 0,
        NAMECOLOR_NPC = 1,
        NAMECOLOR_PC = 2,
        NAMECOLOR_PC_END = NAMECOLOR_PC + EMPIRE_NUM,
        NAMECOLOR_NORMAL_MOB = 7,
        NAMECOLOR_NORMAL_NPC = 8,
        NAMECOLOR_NORMAL_PC = 9,
        NAMECOLOR_NORMAL_PC_END = NAMECOLOR_NORMAL_PC + EMPIRE_NUM,
        NAMECOLOR_EMPIRE_MOB = 14,
        NAMECOLOR_EMPIRE_NPC = 15,
        NAMECOLOR_EMPIRE_PC = 16,
        NAMECOLOR_EMPIRE_PC_END = NAMECOLOR_EMPIRE_PC + EMPIRE_NUM,
        NAMECOLOR_FUNC = 21,
        NAMECOLOR_PK = 22,
        NAMECOLOR_PVP = 23,
        NAMECOLOR_PARTY = 24,
        NAMECOLOR_WARP = 25,
        NAMECOLOR_WAYPOINT = 26,
        NAMECOLOR_EXTRA = NAMECOLOR_FUNC + 10,
        NAMECOLOR_NUM = NAMECOLOR_EXTRA + 10,
    };

    enum
    {
        ALIGNMENT_TYPE_WHITE,
        ALIGNMENT_TYPE_NORMAL,
        ALIGNMENT_TYPE_DARK,
    };

    enum
    {
        EMOTICON_EXCLAMATION = 1,
        EMOTICON_FISH = 11,
        EMOTICON_NUM = 128,

        TITLE_NUM = 9,
        TITLE_NONE = 4,
    };

    enum // 아래 번호가 바뀌면 registerEffect 쪽도 바꾸어 줘야 한다.
    {
        EFFECT_REFINED_NONE,

        EFFECT_SWORD_REFINED7,
        EFFECT_SWORD_REFINED8,
        EFFECT_SWORD_REFINED9,

        EFFECT_BOW_REFINED7,
        EFFECT_BOW_REFINED8,
        EFFECT_BOW_REFINED9,

        EFFECT_FANBELL_REFINED7,
        EFFECT_FANBELL_REFINED8,
        EFFECT_FANBELL_REFINED9,

        EFFECT_SMALLSWORD_REFINED7,
        EFFECT_SMALLSWORD_REFINED8,
        EFFECT_SMALLSWORD_REFINED9,

        EFFECT_SMALLSWORD_REFINED7_LEFT,
        EFFECT_SMALLSWORD_REFINED8_LEFT,
        EFFECT_SMALLSWORD_REFINED9_LEFT,

        EFFECT_BODYARMOR_REFINED7,
        EFFECT_BODYARMOR_REFINED8,
        EFFECT_BODYARMOR_REFINED9,

        EFFECT_BODYARMOR_SPECIAL,  // 갑옷 4-2-1
        EFFECT_BODYARMOR_SPECIAL2, // 갑옷 4-2-2

        EFFECT_REFINED_NUM,
    };

    enum DamageFlag
    {
        DAMAGE_NORMAL = (1 << 0),
        DAMAGE_POISON = (1 << 1),
        DAMAGE_DODGE = (1 << 2),
        DAMAGE_BLOCK = (1 << 3),
        DAMAGE_PENETRATE = (1 << 4),
        DAMAGE_CRITICAL = (1 << 5),
        // 반-_-사
    };

    enum
    {
        EFFECT_DUST = 0,
        EFFECT_STUN = 1,
        EFFECT_HIT = 2,
        EFFECT_FLAME_ATTACK = 3,
        EFFECT_FLAME_HIT = 4,
        EFFECT_FLAME_ATTACH = 5,
        EFFECT_ELECTRIC_ATTACK = 6,
        EFFECT_ELECTRIC_HIT = 7,
        EFFECT_ELECTRIC_ATTACH = 8,
        EFFECT_SPAWN_APPEAR = 9,
        EFFECT_SPAWN_DISAPPEAR = 10,
        EFFECT_LEVELUP = 11,
        EFFECT_SKILLUP = 12,
        EFFECT_HPUP_RED = 13,
        EFFECT_SPUP_BLUE = 14,
        EFFECT_SPEEDUP_GREEN = 15,
        EFFECT_DXUP_PURPLE = 16,
        EFFECT_CRITICAL = 17,
        EFFECT_PENETRATE = 18,
        EFFECT_BLOCK = 19,
        EFFECT_DODGE = 20,
        EFFECT_FIRECRACKER = 21,
        EFFECT_SPIN_TOP = 22,
        EFFECT_WEAPON = 23,
        EFFECT_WEAPON_END = EFFECT_WEAPON + WEAPON_NUM,
        EFFECT_AFFECT = 27,
        EFFECT_AFFECT_GYEONGGONG = EFFECT_AFFECT + AFFECT_GYEONGGONG,
        EFFECT_AFFECT_KWAESOK = EFFECT_AFFECT + AFFECT_KWAESOK,
        EFFECT_AFFECT_END = EFFECT_AFFECT + AFFECT_NUM,
        EFFECT_EMOTICON = 92,
        EFFECT_EMOTICON_END = EFFECT_EMOTICON + EMOTICON_NUM,
        EFFECT_SELECT = 221,
        EFFECT_TARGET = 222,
        EFFECT_EMPIRE = 223,
        EFFECT_EMPIRE_END = EFFECT_EMPIRE + EMPIRE_NUM,
        EFFECT_HORSE_DUST = 228,
        EFFECT_REFINED = 229,
        EFFECT_REFINED_END = EFFECT_REFINED + EFFECT_REFINED_NUM,
        EFFECT_DAMAGE_TARGET = 251,
        EFFECT_DAMAGE_NOT_TARGET = 252,
        EFFECT_DAMAGE_SELFDAMAGE = 253,
        EFFECT_DAMAGE_SELFDAMAGE2 = 254,
        EFFECT_DAMAGE_POISON = 255,
        EFFECT_DAMAGE_MISS = 256,
        EFFECT_DAMAGE_TARGETMISS = 257,
        EFFECT_DAMAGE_CRITICAL = 258,
        EFFECT_SUCCESS = 259,
        EFFECT_FAIL = 260,
        EFFECT_FR_SUCCESS = 261,
        EFFECT_LEVELUP_ON_14_FOR_GERMANY = 262,    // 레벨업 14일때 ( 독일전용 )
        EFFECT_LEVELUP_UNDER_15_FOR_GERMANY = 263, // 레벨업 15일때 ( 독일전용 )
        EFFECT_PERCENT_DAMAGE1 = 264,
        EFFECT_PERCENT_DAMAGE2 = 265,
        EFFECT_PERCENT_DAMAGE3 = 266,
        EFFECT_AUTO_HPUP = 267,
        EFFECT_AUTO_SPUP = 268,
        EFFECT_RAMADAN_RING_EQUIP = 269,    // 초승달 반지 착용 순간에 발동하는 이펙트
        EFFECT_HALLOWEEN_CANDY_EQUIP = 270, // 할로윈 사탕 착용 순간에 발동하는 이펙트
        EFFECT_HAPPINESS_RING_EQUIP = 271,  // 행복의 반지 착용 순간에 발동하는 이펙트
        EFFECT_LOVE_PENDANT_EQUIP = 272,    // 행복의 반지 착용 순간에 발동하는 이펙트
        EFFECT_TEMP = 273,
        EFFECT_NUM = 274,
    };

    enum
    {
        DUEL_NONE,
        DUEL_CANNOTATTACK,
        DUEL_START,
    };

    static void DestroySystem();
    static void CreateSystem(UINT uCapacity);
    static bool RegisterEffect(UINT eEftType, const char *c_szEftAttachBone, const char *c_szEftName, bool isCache);
    static void RegisterTitleName(int iIndex, const char *c_szTitleName);
    static bool RegisterNameColor(UINT uIndex, UINT r, UINT g, UINT b);
    static bool RegisterTitleColor(UINT uIndex, UINT r, UINT g, UINT b);
    static bool ChangeEffectTexture(UINT eEftType, const char *c_szSrcFileName, const char *c_szDstFileName);

    static void SetDustGap(float fDustGap);
    static void SetHorseDustGap(float fDustGap);

    static void SetEmpireNameMode(bool isEnable);
    static const D3DXCOLOR &GetIndexedNameColor(UINT eNameColor);

    void SetMainInstance();

    void OnSelected();
    void OnUnselected();
    void OnTargeted();
    void OnUntargeted();

  protected:
    bool __IsExistMainInstance();
    bool __IsMainInstance();
    bool __MainCanSeeHiddenThing();
    float __GetBowRange();

    DWORD __AttachEffect(UINT eEftType);
    DWORD __AttachEffect(char filename[128]);
    void __DetachEffect(DWORD dwEID);

  public:
    void CreateSpecialEffect(DWORD iEffectIndex);
    void AttachSpecialEffect(DWORD effect);

  protected:
    static std::string ms_astAffectEffectAttachBone[EFFECT_NUM];
    static DWORD ms_adwCRCAffectEffect[EFFECT_NUM];
    static float ms_fDustGap;
    static float ms_fHorseDustGap;

  public:
    CInstanceBase();
    virtual ~CInstanceBase();

    bool LessRenderOrder(CInstanceBase *pkInst);

    void MountHorse(UINT eRace);
    void DismountHorse();

    // 스크립트용 테스트 함수. 나중에 없에자
    void SCRIPT_SetAffect(UINT eAffect, bool isVisible);

    float CalculateDistanceSq3d(const TPixelPosition &c_rkPPosDst);

    // Instance Data
    bool IsFlyTargetObject();
    void ClearFlyTargetInstance();
    void SetFlyTargetInstance(CInstanceBase &rkInstDst);
    void AddFlyTargetInstance(CInstanceBase &rkInstDst);
    void AddFlyTargetPosition(const TPixelPosition &c_rkPPosDst);

    float GetFlyTargetDistance();

    void SetAlpha(float fAlpha);

    void DeleteBlendOut();

    void AttachTextTail();
    void DetachTextTail();
    void UpdateTextTailLevel(DWORD level);

    void RefreshTextTail();
    void RefreshTextTailTitle();

    bool Create(const SCreateData &c_rkCreateData);

    bool CreateDeviceObjects();
    void DestroyDeviceObjects();

    void Destroy();

    void Update();
    bool UpdateDeleting();

    void Transform();
    void Deform();
    void Render();
    void RenderTrace();
    void RenderToShadowMap();
    void RenderCollision();
    void RegisterBoundingSphere();

    // Temporary
    void GetBoundBox(D3DXVECTOR3 *vtMin, D3DXVECTOR3 *vtMax);

    void SetNameString(const char *c_szName, int len);
    bool SetRace(DWORD eRace);
    void SetVirtualID(DWORD dwVirtualID);
    void SetVirtualNumber(DWORD dwVirtualNumber);
    void SetInstanceType(int iInstanceType);
    void SetAlignment(short sAlignment);
    void SetPKMode(BYTE byPKMode);
    void SetKiller(bool bFlag);
    void SetPartyMemberFlag(bool bFlag);
    void SetStateFlags(DWORD dwStateFlags);

    void SetArmor(DWORD dwArmor);
    void SetShape(DWORD eShape, float fSpecular = 0.0f);
    void SetHair(DWORD eHair);
    bool SetWeapon(DWORD eWeapon);
    bool ChangeArmor(DWORD dwArmor);
    void ChangeWeapon(DWORD eWeapon);
    void ChangeHair(DWORD eHair);
    void ChangeGuild(DWORD dwGuildID);
    DWORD GetWeaponType();

    void SetComboType(UINT uComboType);
    void SetAttackSpeed(UINT uAtkSpd);
    void SetMoveSpeed(UINT uMovSpd);
    void SetRotationSpeed(float fRotSpd);

    const char *GetNameString();
    int GetInstanceType();
    DWORD GetPart(CRaceData::EParts part);
    DWORD GetShape() const;
    DWORD GetRace() const;
    DWORD GetVirtualID();
    DWORD GetVirtualNumber() const;
    DWORD GetEmpireID() const;
    DWORD GetGuildID() const;
    int GetAlignment() const;
    UINT GetAlignmentGrade() const;
    int GetAlignmentType();
    BYTE GetPKMode() const;
    bool IsKiller() const;
    bool IsPartyMember() const;

    void ActDualEmotion(CInstanceBase &rkDstInst, WORD dwMotionNumber1, WORD dwMotionNumber2);
    void ActEmotion(DWORD dwMotionNumber);
    void LevelUp();
    void SkillUp();
    void UseSpinTop();
    void Revive();
    void Stun();
    void Die();
    void Hide();
    void Show();

    bool CanAct();
    bool CanMove();
    bool CanAttack();
    bool CanUseSkill();
    bool CanFishing();
    bool IsConflictAlignmentInstance(CInstanceBase &rkInstVictim);
    bool IsAttackableInstance(CInstanceBase &rkInstVictim);
    bool IsTargetableInstance(CInstanceBase &rkInstVictim);
    bool IsPVPInstance(CInstanceBase &rkInstSel);
    bool CanChangeTarget();
    bool CanPickInstance();
    bool CanViewTargetHP(CInstanceBase &rkInstVictim);

    // Movement
    BOOL IsGoing() const;
    bool NEW_Goto(const TPixelPosition &c_rkPPosDst, float fDstRot);
    void EndGoing();

    void SetRunMode();
    void SetWalkMode();

    bool IsAffect(UINT uAffect);
    BOOL IsInvisibility();
    BOOL IsParalysis();
    BOOL IsGameMaster();
    BOOL IsSameEmpire(CInstanceBase &rkInstDst);
    BOOL IsBowMode();
    BOOL IsHandMode();
    BOOL IsFishingMode();
    BOOL IsFishing();

    BOOL IsWearingDress() const;
    BOOL IsHoldingPickAxe();
    BOOL IsMountingHorse();
    BOOL IsNewMount();
    BOOL IsForceVisible();
    BOOL IsInSafe();
    BOOL IsEnemy();
    BOOL IsStone();
    BOOL IsResource() const;
    BOOL IsNPC();
    BOOL IsPC();
    BOOL IsPoly();
    BOOL IsWarp();
    BOOL IsGoto();
    BOOL IsObject();
    BOOL IsDoor();
    BOOL IsBuilding();
    BOOL IsWoodenDoor();
    BOOL IsStoneDoor();
    BOOL IsFlag();
    BOOL IsGuildWall() const;

    BOOL IsDead();
    BOOL IsStun();
    BOOL IsSleep();
    BOOL __IsSyncing();
    BOOL IsWaiting();
    BOOL IsWalking();
    BOOL IsPushing();
    BOOL IsAttacking();
    BOOL IsActingEmotion();
    BOOL IsAttacked();
    BOOL IsKnockDown();
    BOOL IsUsingSkill();
    BOOL IsUsingMovingSkill();
    BOOL CanCancelSkill();
    BOOL CanAttackHorseLevel();

#ifdef __MOVIE_MODE__
    BOOL IsMovieMode(); // 운영자용 완전히 안보이는거
#endif
    bool NEW_CanMoveToDestPixelPosition(const TPixelPosition &c_rkPPosDst);

    void NEW_SetAdvancingRotationFromPixelPosition(const TPixelPosition &c_rkPPosSrc,
                                                   const TPixelPosition &c_rkPPosDst);
    void NEW_SetAdvancingRotationFromDirPixelPosition(const TPixelPosition &c_rkPPosDir);
    bool NEW_SetAdvancingRotationFromDestPixelPosition(const TPixelPosition &c_rkPPosDst);
    void SetAdvancingRotation(float fRotation);

    void EndWalking(float fBlendingTime = 0.15f);
    void EndWalkingWithoutBlending();

    // Battle
    void SetEventHandler(CActorInstance::IEventHandler *pkEventHandler);

    void PushUDPState(DWORD dwCmdTime, const TPixelPosition &c_rkPPosDst, float fDstRot, UINT eFunc, UINT uArg);
    void PushTCPState(DWORD dwCmdTime, const TPixelPosition &c_rkPPosDst, float fDstRot, UINT eFunc, UINT uArg);
    void PushTCPStateExpanded(DWORD dwCmdTime, const TPixelPosition &c_rkPPosDst, float fDstRot, UINT eFunc, UINT uArg,
                              UINT uTargetVID);

    void NEW_Stop();

    bool NEW_UseSkill(UINT uSkill, UINT uMot, UINT uMotLoopCount, bool isMovingSkill);
    void NEW_Attack();
    void NEW_Attack(float fDirRot);
    void NEW_AttackToDestPixelPositionDirection(const TPixelPosition &c_rkPPosDst);
    bool NEW_AttackToDestInstanceDirection(CInstanceBase &rkInstDst, IFlyEventHandler *pkFlyHandler);
    bool NEW_AttackToDestInstanceDirection(CInstanceBase &rkInstDst);

    bool NEW_MoveToDestPixelPositionDirection(const TPixelPosition &c_rkPPosDst);
    void NEW_MoveToDestInstanceDirection(CInstanceBase &rkInstDst);
    void NEW_MoveToDirection(float fDirRot);

    float NEW_GetDistanceFromDirPixelPosition(const TPixelPosition &c_rkPPosDir);
    float NEW_GetDistanceFromDestPixelPosition(const TPixelPosition &c_rkPPosDst);
    float NEW_GetDistanceFromDestInstance(CInstanceBase &rkInstDst);

    float NEW_GetRotation();
    float NEW_GetRotationFromDestPixelPosition(const TPixelPosition &c_rkPPosDst);
    float NEW_GetRotationFromDirPixelPosition(const TPixelPosition &c_rkPPosDir);
    float NEW_GetRotationFromDestInstance(CInstanceBase &rkInstDst);

    float NEW_GetAdvancingRotationFromDirPixelPosition(const TPixelPosition &c_rkPPosDir);
    float NEW_GetAdvancingRotationFromDestPixelPosition(const TPixelPosition &c_rkPPosDst);
    float NEW_GetAdvancingRotationFromPixelPosition(const TPixelPosition &c_rkPPosSrc,
                                                    const TPixelPosition &c_rkPPosDst);

    BOOL NEW_IsClickableDistanceDestPixelPosition(const TPixelPosition &c_rkPPosDst);
    BOOL NEW_IsClickableDistanceDestInstance(CInstanceBase &rkInstDst);

    bool NEW_GetFrontInstance(CInstanceBase **ppoutTargetInstance, float fDistance);
    void NEW_GetRandomPositionInFanRange(CInstanceBase &rkInstTarget, TPixelPosition *pkPPosDst);
    bool NEW_GetInstanceVectorInFanRange(float fSkillDistance, CInstanceBase &rkInstTarget,
                                         std::vector<CInstanceBase *> *pkVct_pkInst);
    bool NEW_GetInstanceVectorInCircleRange(float fSkillDistance, std::vector<CInstanceBase *> *pkVct_pkInst);

    void NEW_SetOwner(DWORD dwVIDOwner);
    void NEW_SyncPixelPosition(long nPPosX, long nPPosY);
    void NEW_SyncCurrentPixelPosition();

    void NEW_SetPixelPosition(const TPixelPosition &c_rPixelPosition);

    bool NEW_IsLastPixelPosition();
    const TPixelPosition &NEW_GetLastPixelPositionRef();

    // Battle
    BOOL isNormalAttacking();
    BOOL isComboAttacking();
    MOTION_KEY GetNormalAttackIndex();
    DWORD GetComboIndex();
    float GetAttackingElapsedTime();
    void InputNormalAttack(float fAtkDirRot);
    void InputComboAttack(float fAtkDirRot);

    void RunNormalAttack(float fAtkDirRot);
    void RunComboAttack(float fAtkDirRot, DWORD wMotionIndex);

    CInstanceBase *FindNearestVictim();
    BOOL CheckAdvancing();

    bool AvoidObject(const CGraphicObjectInstance &c_rkBGObj);
    bool IsBlockObject(const CGraphicObjectInstance &c_rkBGObj);
    void BlockMovement();

    BOOL CheckAttacking(CInstanceBase &rkInstVictim);
    void ProcessHitting(DWORD dwMotionKey, CInstanceBase *pVictimInstance);
    void ProcessHitting(DWORD dwMotionKey, BYTE byEventIndex, CInstanceBase *pVictimInstance);
    void GetBlendingPosition(TPixelPosition *pPixelPosition);
    void SetBlendingPosition(const TPixelPosition &c_rPixelPosition);

    // Fishing
    void StartFishing(float frot);
    void StopFishing();
    void ReactFishing();
    void CatchSuccess();
    void CatchFail();
    BOOL GetFishingRot(int *pirot);

    // Render Mode
    void RestoreRenderMode();
    void SetAddRenderMode();
    void SetModulateRenderMode();
    void SetRenderMode(int iRenderMode);
    void SetAddColor(const D3DXCOLOR &c_rColor);

    // Position
    void SCRIPT_SetPixelPosition(float fx, float fy);
    void NEW_GetPixelPosition(TPixelPosition *pPixelPosition);

    // Rotation
    void NEW_LookAtFlyTarget();
    void NEW_LookAtDestInstance(CInstanceBase &rkInstDst);
    void NEW_LookAtDestPixelPosition(const TPixelPosition &c_rkPPosDst);

    float GetRotation();
    float GetAdvancingRotation();
    void SetRotation(float fRotation);
    void BlendRotation(float fRotation, float fBlendTime = 0.1f);

    void SetDirection(int dir);
    void BlendDirection(int dir, float blendTime);
    float GetDegreeFromDirection(int dir);

    // Motion
    //	Motion Deque
    BOOL isLock();

    void SetMotionMode(int iMotionMode);
    int GetMotionMode(DWORD dwMotionIndex);

    // Motion
    //	Pushing Motion
    void ResetLocalTime();
    void SetLoopMotion(WORD wMotion, float fBlendTime = 0.1f, float fSpeedRatio = 1.0f);
    void PushOnceMotion(WORD wMotion, float fBlendTime = 0.1f, float fSpeedRatio = 1.0f);
    void PushLoopMotion(WORD wMotion, float fBlendTime = 0.1f, float fSpeedRatio = 1.0f);
    void SetEndStopMotion();

    // Intersect
    bool IntersectDefendingSphere();
    bool IntersectBoundingBox();

    // Part
    // void					SetParts(const WORD * c_pParts);
    void Refresh(DWORD dwMotIndex, bool isLoop);

    // void					AttachEffectByID(DWORD
    // dwParentPartIndex, const char * c_pszBoneName, DWORD dwEffectID, int dwLife
    // = CActorInstance::EFFECT_LIFE_INFINITE ); // 수명은 ms단위입니다. void
    // AttachEffectByName(DWORD dwParentPartIndex, const char * c_pszBoneName,
    // const char * c_pszEffectName, int dwLife =
    // CActorInstance::EFFECT_LIFE_INFINITE ); // 수명은 ms단위입니다.

    float GetDistance(CInstanceBase *pkTargetInst);
    float GetDistance(const TPixelPosition &c_rPixelPosition);

    // ETC
    CActorInstance &GetGraphicThingInstanceRef();
    CActorInstance *GetGraphicThingInstancePtr();

    bool __Background_IsWaterPixelPosition(const TPixelPosition &c_rkPPos);
    bool __Background_GetWaterHeight(const TPixelPosition &c_rkPPos, float *pfHeight);

    // 2004.07.25.myevan.이펙트 안나오는 문제
    /////////////////////////////////////////////////////////////
    void __ClearAffectFlagContainer();
    void __ClearAffects();
    /////////////////////////////////////////////////////////////

    void __SetAffect(UINT eAffect, bool isVisible);

    void SetAffectFlagContainer(const CAffectFlagContainer &c_rkAffectFlagContainer);

    void __SetNormalAffectFlagContainer(const CAffectFlagContainer &c_rkAffectFlagContainer);
    void __SetStoneSmokeFlagContainer(const CAffectFlagContainer &c_rkAffectFlagContainer);

    void SetEmoticon(UINT eEmoticon);
    void SetFishEmoticon();
    bool IsPossibleEmoticon() const;

  protected:
    UINT __LessRenderOrder_GetLODLevel();
    void __Initialize();
    void __InitializeRotationSpeed();

    void __Create_SetName(const SCreateData &c_rkCreateData);
    void __Create_SetWarpName(const SCreateData &c_rkCreateData);

    CInstanceBase *__GetMainInstancePtr();
    CInstanceBase *__FindInstancePtr(DWORD dwVID);

    bool __FindRaceType(DWORD dwRace, BYTE *pbType);
    DWORD __GetRaceType() const;

    bool __IsShapeAnimalWear();
    BOOL __IsChangableWeapon(int iWeaponID);

    void __EnableSkipCollision();
    void __DisableSkipCollision();

    void __ClearMainInstance();

    void __Shaman_SetParalysis(bool isParalysis);
    void __Warrior_SetGeomgyeongAffect(bool isVisible);
    void __Assassin_SetEunhyeongAffect(bool isVisible);
    void __SetReviveInvisibilityAffect(bool isVisible);

    BOOL __CanProcessNetworkStatePacket();

    bool __IsInDustRange();

    // Emotion
    void __ProcessFunctionEmotion(DWORD dwMotionNumber, DWORD dwTargetVID, const TPixelPosition &c_rkPosDst);
    void __EnableChangingTCPState();
    void __DisableChangingTCPState();
    BOOL __IsEnableTCPProcess(UINT eCurFunc);

    // 2004.07.17.levites.isShow를 ViewFrustumCheck로 변경
    bool __CanRender();
    bool __IsInViewFrustum();

    // HORSE
    void __AttachHorseSaddle();
    void __DetachHorseSaddle();

    struct SHORSE
    {
        bool m_isMounting = false;
        CActorInstance *m_pkActor = nullptr;

        SHORSE();
        ~SHORSE();

        void Destroy();
        void Create(const TPixelPosition &c_rkPPos, UINT eRace, UINT eHitEffect);

        void SetAttackSpeed(UINT uAtkSpd);
        void SetMoveSpeed(UINT uMovSpd);
        void Deform();
        void Render();
        CActorInstance &GetActorRef() const;
        CActorInstance *GetActorPtr() const;

        bool IsMounting() const;
        bool CanAttack();
        bool CanUseSkill();

        UINT GetLevel() const;
        bool IsNewMount() const;

        void __Initialize();
    } m_kHorse;

    // Blend Mode
    void __SetBlendRenderingMode();
    void __SetAlphaValue(float fAlpha);
    float __GetAlphaValue();

    void __ComboProcess();
    void MovementProcess();
    void TodoProcess();
    void StateProcess();
    void AttackProcess();

    void StartWalking();
    float GetLocalTime();

    void RefreshState(DWORD dwMotIndex, bool isLoop);
    void RefreshActorInstance();

    void OnSyncing();
    void OnWaiting();
    void OnMoving();

    void NEW_SetCurPixelPosition(const TPixelPosition &c_rkPPosDst);
    void NEW_SetSrcPixelPosition(const TPixelPosition &c_rkPPosSrc);
    void NEW_SetDstPixelPosition(const TPixelPosition &c_rkPPosDst);
    void NEW_SetDstPixelPositionZ(FLOAT z);

    const TPixelPosition &NEW_GetCurPixelPositionRef();
    const TPixelPosition &NEW_GetSrcPixelPositionRef();

  public:
    const TPixelPosition &NEW_GetDstPixelPositionRef();

  protected:
    BOOL m_isTextTail = 0;

    // Instance Data
    std::string m_stName;

    DWORD m_awPart[CRaceData::PART_MAX_NUM]{};

    DWORD m_dwLevel = 0U;
    DWORD m_dwEmpireID = 0U;
    DWORD m_dwGuildID = 0U;

    CAffectFlagContainer m_kAffectFlagContainer;
    DWORD m_adwCRCAffectEffect[AFFECT_NUM]{};

    UINT __GetRefinedEffect(CItemData *pItem);
    void __ClearWeaponRefineEffect();
    void __ClearArmorRefineEffect();

    void __AttachSelectEffect();
    void __DetachSelectEffect();

    void __AttachTargetEffect();
    void __DetachTargetEffect();

    void __AttachEmpireEffect(DWORD eEmpire);

    struct SEffectContainer
    {
        using Dict = std::map<DWORD, DWORD>;
        Dict m_kDct_dwEftID;
    } m_kEffectContainer;

    void __EffectContainer_Initialize();
    void __EffectContainer_Destroy();

    DWORD __EffectContainer_AttachEffect(DWORD dwEftKey);
    void __EffectContainer_DetachEffect(DWORD dwEftKey);

    SEffectContainer::Dict &__EffectContainer_GetDict();

    struct SStoneSmoke
    {
        DWORD m_dwEftID;
    } m_kStoneSmoke{};

    void __StoneSmoke_Inialize();
    void __StoneSmoke_Destroy();
    void __StoneSmoke_Create(DWORD eSmoke);

    // Emoticon
    // DWORD m_adwCRCEmoticonEffect[EMOTICON_NUM];

    BYTE m_eType = 0U;
    BYTE m_eRaceType = 0U;
    DWORD m_eShape = 0U;
    DWORD m_dwRace = 0U;
    DWORD m_dwVirtualNumber = 0U;
    short m_sAlignment = 0;
    BYTE m_byPKMode = 0U;
    bool m_isKiller = false;
    bool m_isPartyMember = false;

    // Movement
    int m_iRotatingDirection = 0;

    DWORD m_dwAdvActorVID = 0U;
    DWORD m_dwLastDmgActorVID = 0U;

    LONG m_nAverageNetworkGap = 0;
    DWORD m_dwNextUpdateHeightTime = 0U;

    bool m_isGoing = false;

    TPixelPosition m_kPPosDust;

    DWORD m_dwLastComboIndex = 0U;

    DWORD m_swordRefineEffectRight = 0U;
    DWORD m_swordRefineEffectLeft = 0U;
    DWORD m_armorRefineEffect = 0U;

    struct SMoveAfterFunc
    {
        UINT eFunc = 0U;
        UINT uArg = 0U;

        // For Emotion Function
        UINT uArgExpanded = 0U;
        TPixelPosition kPosDst;
    };

    SMoveAfterFunc m_kMovAfterFunc;

    float m_fDstRot = 0.0F;
    float m_fAtkPosTime = 0.0F;
    float m_fRotSpd = 0.0F;
    float m_fMaxRotSpd = 0.0F;

    BOOL m_bEnableTCPState = 0;

    // Graphic Instance
    CActorInstance m_GraphicThingInstance;

    struct SCommand
    {
        DWORD m_dwChkTime = 0U;
        DWORD m_dwCmdTime = 0U;
        float m_fDstRot = 0.0F;
        UINT m_eFunc = 0U;
        UINT m_uArg = 0U;
        UINT m_uTargetVID = 0U;
        TPixelPosition m_kPPosDst;
    };

    using CommandQueue = std::list<SCommand>;

    DWORD m_dwBaseChkTime = 0U;
    DWORD m_dwBaseCmdTime = 0U;

    DWORD m_dwSkipTime = 0U;

    CommandQueue m_kQue_kCmdNew;

    BOOL m_bDamageEffectType = 0;

    struct SEffectDamage
    {
        DWORD damage;
        BYTE flag;
        BOOL bSelf;
        BOOL bTarget;
    };

    using CommandDamageQueue = std::list<SEffectDamage>;
    CommandDamageQueue m_DamageQueue;

    void ProcessDamage();

  public:
    void AddDamageEffect(DWORD damage, BYTE flag, BOOL bSelf, BOOL bTarget);

  protected:
    struct SWarrior
    {
        DWORD m_dwGeomgyeongEffect;
    };

    SWarrior m_kWarrior{};

    void __Warrior_Initialize();

  public:
    static void ClearPVPKeySystem();

    static void InsertPVPKey(DWORD dwVIDSrc, DWORD dwVIDDst);
    static void InsertPVPReadyKey(DWORD dwVIDSrc, DWORD dwVIDDst);
    static void RemovePVPKey(DWORD dwVIDSrc, DWORD dwVIDDst);

    static void InsertGVGKey(DWORD dwSrcGuildVID, DWORD dwDstGuildVID);
    static void RemoveGVGKey(DWORD dwSrcGuildVID, DWORD dwDstGuildVID);

    static void InsertDUELKey(DWORD dwVIDSrc, DWORD dwVIDDst);

    UINT GetNameColorIndex();

    const D3DXCOLOR &GetNameColor();
    const D3DXCOLOR &GetTitleColor();

  protected:
    static DWORD __GetPVPKey(DWORD dwVIDSrc, DWORD dwVIDDst);
    static bool __FindPVPKey(DWORD dwVIDSrc, DWORD dwVIDDst);
    static bool __FindPVPReadyKey(DWORD dwVIDSrc, DWORD dwVIDDst);
    static bool __FindGVGKey(DWORD dwSrcGuildID, DWORD dwDstGuildID);
    static bool __FindDUELKey(DWORD dwVIDSrc, DWORD dwVIDDst);

    CActorInstance::IEventHandler *GetEventHandlerPtr();
    CActorInstance::IEventHandler &GetEventHandlerRef();

    static float __GetBackgroundHeight(float x, float y);
    static DWORD __GetShadowMapColor(float x, float y);

  public:
    static void ResetPerformanceCounter();
    static void GetInfo(std::string *pstInfo);

    static CInstanceBase *New();
    static void Delete(CInstanceBase *pkInst);

    static CDynamicPool<CInstanceBase> ms_kPool;

  protected:
    static DWORD ms_dwUpdateCounter;
    static DWORD ms_dwRenderCounter;
    static DWORD ms_dwDeformCounter;

  public:
    DWORD GetDuelMode() const;
    void SetDuelMode(DWORD type);

  protected:
    DWORD m_dwDuelMode = 0U;
    DWORD m_dwEmoticonTime = 0U;
};

inline int RaceToJob(int race)
{
    const int JOB_NUM = 4;
    return race % JOB_NUM;
}

inline int RaceToSex(int race)
{
    switch (race)
    {
    case 0:
    case 2:
    case 5:
    case 7:
        return 1;
    case 1:
    case 3:
    case 4:
    case 6:
        return 0;
    }
    return 0;
}
