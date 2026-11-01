#ifndef __MapType_Header__
#define __MapType_Header__

class CProperty;

#include <Neoeter/Render/SkyBox.h>
#include <Neoeter/Audio/SoundManager.h>

/////////////////////////////////////////////////////////////////
// Property
namespace prt
{
enum EPropertyType
{
    PROPERTY_TYPE_NONE,
    PROPERTY_TYPE_TREE,
    PROPERTY_TYPE_BUILDING,
    PROPERTY_TYPE_EFFECT,
    PROPERTY_TYPE_AMBIENCE,
    PROPERTY_TYPE_DUNGEON_BLOCK,
    PROPERTY_TYPE_MAX_NUM,
};

const char c_szPropertyTypeName[PROPERTY_TYPE_MAX_NUM][32] = {"None",   "Tree",     "Building",
                                                              "Effect", "Ambience", "DungeonBlock"};

const char c_szPropertyExtension[PROPERTY_TYPE_MAX_NUM][16] = {".pr", ".prt", ".prb", ".pre", ".pra", ".prd"};

DWORD GetPropertyType(const char *c_szTypeName);
const char *GetPropertyExtension(DWORD dwType);

struct TPropertyTree
{
    std::string strName;
    std::string strFileName;

    float fSize;
    float fVariance;
};

struct TPropertyBuilding
{
    std::string strName;
    std::string strFileName;
    std::string strAttributeDataFileName;
    BOOL isShadowFlag = 0;
};

struct TPropertyEffect
{
    std::string strName;
    std::string strFileName;
};

enum EAmbiencePlayType
{
    AMBIENCE_PLAY_TYPE_ONCE,
    AMBIENCE_PLAY_TYPE_STEP,
    AMBIENCE_PLAY_TYPE_LOOP,
    AMBIENCE_PLAY_TYPE_MAX_NUM,
};

const char c_szPlayTypeName[AMBIENCE_PLAY_TYPE_MAX_NUM][8] = {
    "ONCE",
    "STEP",
    "LOOP",
};

struct TPropertyAmbience
{
    std::string strName;
    std::string strPlayType;

    float fPlayInterval = 0.0F;
    float fPlayIntervalVariation = 0.0F;
    float fMaxVolumeAreaPercentage = 0.0F;
    std::vector<std::string> AmbienceSoundVector;
};

struct TPropertyDungeonBlock
{
    std::string strName;
    std::string strFileName;
    std::string strAttributeDataFileName;
};

bool PropertyTreeDataToString(TPropertyTree *pData, CProperty *pProperty);
bool PropertyTreeStringToData(CProperty *pProperty, TPropertyTree *pData);
bool PropertyBuildingDataToString(TPropertyBuilding *pData, CProperty *pProperty);
bool PropertyBuildingStringToData(CProperty *pProperty, TPropertyBuilding *pData);
bool PropertyEffectDataToString(TPropertyEffect *pData, CProperty *pProperty);
bool PropertyEffectStringToData(CProperty *pProperty, TPropertyEffect *pData);
bool PropertyAmbienceDataToString(TPropertyAmbience *pData, CProperty *pProperty);
bool PropertyAmbienceStringToData(CProperty *pProperty, TPropertyAmbience *pData);
bool PropertyDungeonBlockDataToString(TPropertyDungeonBlock *pData, CProperty *pProperty);
bool PropertyDungeonBlockStringToData(CProperty *pProperty, TPropertyDungeonBlock *pData);
}; // namespace prt
/////////////////////////////////////////////////////////////////

/////////////////////////////////////////////////////////////////
// Environment
//
enum
{
    ENV_DIRLIGHT_BACKGROUND,
    ENV_DIRLIGHT_CHARACTER,
    ENV_DIRLIGHT_NUM
};

using TEnvironmentData = struct SEnvironmentData
{
    // Light
    BOOL bDirLightsEnable[ENV_DIRLIGHT_NUM]{};
    D3DLIGHT9 DirLights[ENV_DIRLIGHT_NUM]{};

    // Material
    D3DMATERIAL9 Material{};

    // Fog
    BOOL bFogEnable = 0;
    BOOL bDensityFog = 0;

    float m_fFogNearDistance = 0.0F;
    float m_fFogFarDistance = 0.0F;

    float GetFogNearDistance() const;
    float GetFogFarDistance() const;

    D3DXCOLOR FogColor;

    // Filtering
    BOOL bFilteringEnable = 0;
    D3DXCOLOR FilteringColor;
    BYTE byFilteringAlphaSrc = 0U;
    BYTE byFilteringAlphaDest = 0U;

    // Wind
    float fWindStrength = 0.0F;
    float fWindRandom = 0.0F;

    // SkyBox
    D3DXVECTOR3 v3SkyBoxScale;
    BOOL bSkyBoxTextureRenderMode = 0;

    BYTE bySkyBoxGradientLevelUpper = 0U;
    BYTE bySkyBoxGradientLevelLower = 0U;

    std::string strSkyBoxFaceFileName[6]; // order : front/back/left/right/top/bottom

    D3DXVECTOR2 v2CloudScale;
    float fCloudHeight = 0.0F;
    D3DXVECTOR2 v2CloudTextureScale;
    D3DXVECTOR2 v2CloudSpeed;
    std::string strCloudTextureFileName;
    TGradientColor CloudGradientColor;

    std::vector<TGradientColor> SkyBoxGradientColorVector;

    // LensFlare
    BOOL bLensFlareEnable = 0;
    D3DXCOLOR LensFlareBrightnessColor;
    float fLensFlareMaxBrightness = 0.0F;

    BOOL bMainFlareEnable = 0;
    std::string strMainFlareTextureFileName;
    float fMainFlareSize = 0.0F;

    BOOL bReserve = 0; // 외부에서 설정을 고치지 않음
};

using TEnvironmentDataMap = std::map<DWORD, TEnvironmentData *>;
/////////////////////////////////////////////////////////////////

using TScreenPosition = struct SScreenPosition
{
    int x;
    int y;
};

using TPixelPosition = D3DXVECTOR3;

float SPixelPosition_CalculateDistanceSq3d(const TPixelPosition &c_rkPPosLeft, const TPixelPosition &c_rkPPosRight);

using TCellPosition = struct SCellPosition
{
    SCellPosition() : x(0), y(0)
    {
    }

    SCellPosition(int x, int y) : x(x), y(y)
    {
    }

    int x, y;
};

const int c_Section_xAttributeCellSize = 50;
const int c_Section_yAttributeCellSize = 50;
const int c_Section_xCellSize = 100;
const int c_Section_yCellSize = 100;

#endif
