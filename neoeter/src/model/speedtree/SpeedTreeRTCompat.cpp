#include <Neoeter/Model/SpeedTree/SpeedTreeRTCompat.h>
#include <Neoeter/Model/SpeedTree/TreeCacheFormat.h>
#include <cstdio>
#include <cctype>
#include <cstdlib>
#include <cstdint>
#include <fstream>
#include <algorithm>

// Static member definitions
std::string   CSpeedTreeRT::ms_strError;
bool          CSpeedTreeRT::ms_bTextureFlip = false;
bool          CSpeedTreeRT::ms_bDropToBillboard = true;
float         CSpeedTreeRT::ms_afCameraPos[3] = {0.0f, 0.0f, 0.0f};
float         CSpeedTreeRT::ms_afCameraDir[3] = {0.0f, 0.0f, -1.0f};
float         CSpeedTreeRT::ms_afLightAttrs[16] = {0.0f};
bool          CSpeedTreeRT::ms_bLightOn = false;
unsigned int  CSpeedTreeRT::ms_unNumWindMatrices = 4;

// SGeometry implementations
CSpeedTreeRT::SGeometry::SIndexed::SIndexed()
    : m_nDiscreteLodLevel(-1),
      m_usNumStrips(0),
      m_pStripLengths(nullptr),
      m_pStrips(nullptr),
      m_usVertexCount(0),
      m_pColors(nullptr),
      m_pNormals(nullptr),
      m_pBinormals(nullptr),
      m_pTangents(nullptr),
      m_pCoords(nullptr),
      m_pTexCoords0(nullptr),
      m_pTexCoords1(nullptr),
      m_pWindWeights(nullptr),
      m_pWindMatrixIndices(nullptr)
{
}

CSpeedTreeRT::SGeometry::SIndexed::~SIndexed()
{
}

CSpeedTreeRT::SGeometry::SLeaf::SLeaf()
    : m_bIsActive(false),
      m_fAlphaTestValue(0.0f),
      m_nDiscreteLodLevel(-1),
      m_usLeafCount(0),
      m_pLeafMapIndices(nullptr),
      m_pLeafClusterIndices(nullptr),
      m_pCenterCoords(nullptr),
      m_pLeafMapTexCoords(nullptr),
      m_pLeafMapCoords(nullptr),
      m_pColors(nullptr),
      m_pNormals(nullptr),
      m_pBinormals(nullptr),
      m_pTangents(nullptr),
      m_pWindWeights(nullptr),
      m_pWindMatrixIndices(nullptr)
{
}

CSpeedTreeRT::SGeometry::SLeaf::~SLeaf()
{
}

CSpeedTreeRT::SGeometry::SBillboard::SBillboard()
    : m_bIsActive(false),
      m_pTexCoords(nullptr),
      m_pCoords(nullptr),
      m_fAlphaTestValue(0.0f)
{
}

CSpeedTreeRT::SGeometry::SBillboard::~SBillboard()
{
}

CSpeedTreeRT::SGeometry::SGeometry()
    : m_fBranchAlphaTestValue(0.0f),
      m_fFrondAlphaTestValue(0.0f)
{
}

CSpeedTreeRT::SGeometry::~SGeometry()
{
}

CSpeedTreeRT::STextures::STextures()
    : m_pBranchTextureFilename(nullptr),
      m_uiLeafTextureCount(0),
      m_pLeafTextureFilenames(nullptr),
      m_uiFrondTextureCount(0),
      m_pFrondTextureFilenames(nullptr),
      m_pCompositeFilename(nullptr),
      m_pSelfShadowFilename(nullptr)
{
}

CSpeedTreeRT::STextures::~STextures()
{
}

CSpeedTreeRT::CSpeedTreeRT()
    : m_fSize(400.0f),
      m_fSizeVariance(0.0f),
      m_nSeed(1),
      m_fWindStrength(0.0f),
      m_fLeafLightingAdjustment(1.0f),
      m_eBranchLighting(LIGHT_STATIC),
      m_eLeafLighting(LIGHT_STATIC),
      m_eFrondLighting(LIGHT_STATIC),
      m_eBranchWind(WIND_NONE),
      m_eLeafWind(WIND_NONE),
      m_eFrondWind(WIND_NONE),
      m_pInstanceOf(nullptr),
      m_eShape(SHAPE_BROADLEAF),
      m_bGeometryGenerated(false),
      m_bBakedGeometry(false),
      m_fBranchAlpha(1.0f),
      m_fFrondAlpha(0.0f),
      m_fLeafAlpha(128.0f),
      m_usBranchStripLength(0),
      m_usFrondStripLength(0)
{
    m_afPos[0] = m_afPos[1] = m_afPos[2] = 0.0f;
    m_afBoundingBox[0] = -100.0f;
    m_afBoundingBox[1] = -100.0f;
    m_afBoundingBox[2] = 0.0f;
    m_afBoundingBox[3] = 100.0f;
    m_afBoundingBox[4] = 100.0f;
    m_afBoundingBox[5] = 600.0f;

    // Default Material: Diffuse(1,1,1), Ambient(0.5,0.5,0.5), Specular(0.2,0.2,0.2), Emissive(0,0,0), Power(5.0)
    const float defaultMat[13] = {
        1.0f, 1.0f, 1.0f,
        0.5f, 0.5f, 0.5f,
        0.2f, 0.2f, 0.2f,
        0.0f, 0.0f, 0.0f,
        5.0f
    };
    std::memcpy(m_afBranchMaterial, defaultMat, sizeof(defaultMat));
    std::memcpy(m_afLeafMaterial, defaultMat, sizeof(defaultMat));
    std::memcpy(m_afFrondMaterial, defaultMat, sizeof(defaultMat));

    // Default trunk collision cylinder
    SCollisionObj obj;
    obj.m_eType = CO_CYLINDER;
    obj.m_afPos[0] = 0.0f;
    obj.m_afPos[1] = 0.0f;
    obj.m_afPos[2] = 0.0f;
    obj.m_afDims[0] = 25.0f;   // Radius
    obj.m_afDims[1] = 200.0f;  // Height
    obj.m_afDims[2] = 0.0f;
    m_vCollisions.push_back(obj);
}

CSpeedTreeRT::~CSpeedTreeRT()
{
}

bool CSpeedTreeRT::Compute(const float* pTransform, unsigned int nSeed, bool bCompositeStrips)
{
    (void)pTransform;
    (void)bCompositeStrips;
    m_nSeed = nSeed;
    if (m_bBakedGeometry)
        return true;
    m_bGeometryGenerated = false;
    GenerateGeometry();
    return true;
}

CSpeedTreeRT* CSpeedTreeRT::Clone(float x, float y, float z, unsigned int nSeed) const
{
    CSpeedTreeRT* pClone = new CSpeedTreeRT(*this);
    pClone->SetTreePosition(x, y, z);
    pClone->m_nSeed = nSeed;
    return pClone;
}

const CSpeedTreeRT* CSpeedTreeRT::InstanceOf(void) const
{
    return m_pInstanceOf;
}

CSpeedTreeRT* CSpeedTreeRT::MakeInstance(void)
{
    CSpeedTreeRT* pInstance = new CSpeedTreeRT();
    pInstance->m_pInstanceOf = this;
    pInstance->m_fSize = m_fSize;
    pInstance->m_fSizeVariance = m_fSizeVariance;
    pInstance->m_eShape = m_eShape;
    pInstance->m_nSeed = m_nSeed;
    pInstance->m_fWindStrength = m_fWindStrength;
    pInstance->m_fLeafLightingAdjustment = m_fLeafLightingAdjustment;
    std::memcpy(pInstance->m_afBoundingBox, m_afBoundingBox, sizeof(m_afBoundingBox));
    std::memcpy(pInstance->m_afBranchMaterial, m_afBranchMaterial, sizeof(m_afBranchMaterial));
    std::memcpy(pInstance->m_afLeafMaterial, m_afLeafMaterial, sizeof(m_afLeafMaterial));
    std::memcpy(pInstance->m_afFrondMaterial, m_afFrondMaterial, sizeof(m_afFrondMaterial));
    pInstance->m_strBranchTexture = m_strBranchTexture;
    pInstance->m_strCompositeTexture = m_strCompositeTexture;
    pInstance->m_strSelfShadowTexture = m_strSelfShadowTexture;
    pInstance->m_vLeafTextures = m_vLeafTextures;
    pInstance->m_vFrondTextures = m_vFrondTextures;
    pInstance->m_vCollisions = m_vCollisions;
    return pInstance;
}

void CSpeedTreeRT::DeleteTransientData(void)
{
}

static std::string ExtractBaseName(const std::string& path)
{
    size_t lastSlash = path.find_last_of("/\\");
    std::string filename = (lastSlash != std::string::npos) ? path.substr(lastSlash + 1) : path;
    return filename;
}

bool CSpeedTreeRT::LoadTree(const unsigned char* pBlock, unsigned int nNumBytes)
{
    if (!pBlock || nNumBytes == 0)
    {
        ms_strError = "Empty SPT block";
        return false;
    }

    // Scan for texture filename strings in the SPT binary
    std::vector<std::string> tokens;
    std::string current;
    for (unsigned int i = 0; i < nNumBytes; ++i)
    {
        char c = static_cast<char>(pBlock[i]);
        if (std::isprint(static_cast<unsigned char>(c)) && c != ' ' && c != '\t' && c != '\r' && c != '\n' && c != '"' && c != '\'')
        {
            current += c;
        }
        else
        {
            if (current.length() >= 4)
            {
                tokens.push_back(current);
            }
            current.clear();
        }
    }
    if (current.length() >= 4)
        tokens.push_back(current);

    for (const auto& rawTok : tokens)
    {
        std::string tok = ExtractBaseName(rawTok);

        // SpeedTree appends a texture-set suffix ("CompositeMapB1#N.tga").
        // The shipped DDS files do not contain it, so drop '#'..'extension'.
        const std::string::size_type hashPos = tok.find('#');
        if (hashPos != std::string::npos)
        {
            const std::string::size_type dotPos = tok.find_last_of('.');
            if (dotPos != std::string::npos && dotPos > hashPos)
                tok = tok.substr(0, hashPos) + tok.substr(dotPos);
            else
                tok = tok.substr(0, hashPos);
        }

        std::string lower;
        for (char ch : tok) lower += static_cast<char>(std::tolower(static_cast<unsigned char>(ch)));

        if (lower.find("compositeshadowmap") != std::string::npos || (lower.find("shadow") != std::string::npos && lower.find(".tga") != std::string::npos))
        {
            // Intentionally ignored: the generated geometry does not provide
            // matching self-shadow texcoords, so leave the stage unbound.
        }
        else if (lower.find("compositemap") != std::string::npos || lower.find("composite") != std::string::npos)
        {
            if (m_strCompositeTexture.empty())
                m_strCompositeTexture = tok;
        }
        else if (lower.find("bark") != std::string::npos || (m_strBranchTexture.empty() && lower.find(".tga") != std::string::npos && lower.find("leaf") == std::string::npos && lower.find("frond") == std::string::npos))
        {
            if (m_strBranchTexture.empty())
                m_strBranchTexture = tok;
        }
        else if (lower.find("leaf") != std::string::npos || lower.find("leaves") != std::string::npos)
        {
            m_vLeafTextures.push_back(tok);
        }
        else if (lower.find("frond") != std::string::npos)
        {
            m_vFrondTextures.push_back(tok);
        }
    }

    // Classify the species from the bark texture name. The SPT geometry
    // generator is proprietary, so the clean-room generator picks an
    // archetype (broadleaf, birch, conifer, palm, baobab, small plant).
    {
        std::string lower;
        for (char ch : m_strBranchTexture)
            lower += static_cast<char>(std::tolower(static_cast<unsigned char>(ch)));

        if (lower.find("palm") != std::string::npos ||
            lower.find("banana") != std::string::npos ||
            lower.find("palmetto") != std::string::npos)
            m_eShape = SHAPE_PALM;
        else if (lower.find("spruce") != std::string::npos ||
                 lower.find("cypress") != std::string::npos ||
                 lower.find("pine") != std::string::npos ||
                 lower.find("cedar") != std::string::npos ||
                 lower.find("juniper") != std::string::npos ||
                 lower.find("christmas") != std::string::npos)
            m_eShape = SHAPE_CONIFER;
        else if (lower.find("baobab") != std::string::npos)
            m_eShape = SHAPE_BAOBAB;
        else if (lower.find("fern") != std::string::npos ||
                 lower.find("aloe") != std::string::npos)
            m_eShape = SHAPE_SMALL;
        else if (lower.find("birch") != std::string::npos)
            m_eShape = SHAPE_BIRCH;
        else
            m_eShape = SHAPE_BROADLEAF;
    }

    // Try the baked geometry cache produced from the original SpeedTreeRT SDK.
    if (LoadBakedGeometry(TreeCache::Fnv1a32(pBlock, nNumBytes)))
        return true;

    // Read the tree height from the parameter stream (property 2001 is the
    // overall tree size in world units, stored as a little-endian float).
    for (unsigned int i = 0; i + 8 <= nNumBytes; ++i)
    {
        if (pBlock[i] == 0xD1 && pBlock[i + 1] == 0x07 && pBlock[i + 2] == 0x00 && pBlock[i + 3] == 0x00)
        {
            float fSize = 0.0f;
            std::memcpy(&fSize, pBlock + i + 4, sizeof(float));
            if (fSize > 20.0f && fSize < 4000.0f)
            {
                SetTreeSize(std::max(200.0f, fSize), 0.0f);
                break;
            }
        }
    }

    return true;
}

bool CSpeedTreeRT::LoadTree(const char* pFilename)
{
    if (!pFilename || !pFilename[0])
    {
        ms_strError = "Invalid tree filename";
        return false;
    }

    FILE* fp = std::fopen(pFilename, "rb");
    if (!fp)
    {
        ms_strError = std::string("Cannot open tree file: ") + pFilename;
        return false;
    }

    std::fseek(fp, 0, SEEK_END);
    long sz = std::ftell(fp);
    std::fseek(fp, 0, SEEK_SET);

    if (sz <= 0)
    {
        std::fclose(fp);
        ms_strError = "Empty tree file";
        return false;
    }

    std::vector<unsigned char> buf(static_cast<size_t>(sz));
    size_t readBytes = std::fread(buf.data(), 1, buf.size(), fp);
    std::fclose(fp);

    return LoadTree(buf.data(), static_cast<unsigned int>(readBytes));
}

unsigned char* CSpeedTreeRT::SaveTree(unsigned int& nNumBytes, bool bSaveLeaves) const
{
    nNumBytes = 0;
    return nullptr;
}

void CSpeedTreeRT::GetTreeSize(float& fSize, float& fVariance) const
{
    fSize = m_fSize;
    fVariance = m_fSizeVariance;
}

void CSpeedTreeRT::SetTreeSize(float fNewSize, float fNewVariance)
{
    m_fSize = fNewSize;
    m_fSizeVariance = fNewVariance;
    if (!m_bBakedGeometry)
        m_bGeometryGenerated = false;
    if (fNewSize > 0.0f)
    {
        m_afBoundingBox[0] = -fNewSize * 0.25f;
        m_afBoundingBox[1] = -fNewSize * 0.25f;
        m_afBoundingBox[2] = 0.0f;
        m_afBoundingBox[3] = fNewSize * 0.25f;
        m_afBoundingBox[4] = fNewSize * 0.25f;
        m_afBoundingBox[5] = fNewSize;
    }
}

unsigned int CSpeedTreeRT::GetSeed() const
{
    return m_nSeed;
}

const float* CSpeedTreeRT::GetTreePosition(void) const
{
    return m_afPos;
}

void CSpeedTreeRT::SetTreePosition(float x, float y, float z)
{
    m_afPos[0] = x;
    m_afPos[1] = y;
    m_afPos[2] = z;
}

void CSpeedTreeRT::SetLeafTargetAlphaMask(unsigned char ucMask)
{
}

CSpeedTreeRT::ELightingMethod CSpeedTreeRT::GetBranchLightingMethod(void) const { return m_eBranchLighting; }
void CSpeedTreeRT::SetBranchLightingMethod(ELightingMethod eMethod) { m_eBranchLighting = eMethod; }
CSpeedTreeRT::ELightingMethod CSpeedTreeRT::GetLeafLightingMethod(void) const { return m_eLeafLighting; }
void CSpeedTreeRT::SetLeafLightingMethod(ELightingMethod eMethod) { m_eLeafLighting = eMethod; }
CSpeedTreeRT::ELightingMethod CSpeedTreeRT::GetFrondLightingMethod(void) const { return m_eFrondLighting; }
void CSpeedTreeRT::SetFrondLightingMethod(ELightingMethod eMethod) { m_eFrondLighting = eMethod; }

CSpeedTreeRT::EStaticLightingStyle CSpeedTreeRT::GetStaticLightingStyle(void) const { return SLS_BASIC; }
void CSpeedTreeRT::SetStaticLightingStyle(EStaticLightingStyle eStyle) {}

float CSpeedTreeRT::GetLeafLightingAdjustment() const { return m_fLeafLightingAdjustment; }
void CSpeedTreeRT::SetLeafLightingAdjustment(float fScalar) { m_fLeafLightingAdjustment = fScalar; }

bool CSpeedTreeRT::GetLightState(unsigned int nLightIndex) { return ms_bLightOn; }
void CSpeedTreeRT::SetLightState(unsigned int nLightIndex, bool bLightOn) { ms_bLightOn = bLightOn; }
const float* CSpeedTreeRT::GetLightAttributes(unsigned int nLightIndex) { return ms_afLightAttrs; }
void CSpeedTreeRT::SetLightAttributes(unsigned int nLightIndex, const float* pLightAttributes)
{
    if (pLightAttributes)
        std::memcpy(ms_afLightAttrs, pLightAttributes, sizeof(ms_afLightAttrs));
}

const float* CSpeedTreeRT::GetBranchMaterial(void) const { return m_afBranchMaterial; }
void CSpeedTreeRT::SetBranchMaterial(const float* pMaterial)
{
    if (pMaterial) std::memcpy(m_afBranchMaterial, pMaterial, sizeof(m_afBranchMaterial));
}

const float* CSpeedTreeRT::GetLeafMaterial(void) const { return m_afLeafMaterial; }
void CSpeedTreeRT::SetLeafMaterial(const float* pMaterial)
{
    if (pMaterial) std::memcpy(m_afLeafMaterial, pMaterial, sizeof(m_afLeafMaterial));
}

const float* CSpeedTreeRT::GetFrondMaterial(void) const { return m_afFrondMaterial; }
void CSpeedTreeRT::SetFrondMaterial(const float* pMaterial)
{
    if (pMaterial) std::memcpy(m_afFrondMaterial, pMaterial, sizeof(m_afFrondMaterial));
}

void CSpeedTreeRT::GetCamera(float* pPosition, float* pDirection)
{
    if (pPosition) std::memcpy(pPosition, ms_afCameraPos, sizeof(ms_afCameraPos));
    if (pDirection) std::memcpy(pDirection, ms_afCameraDir, sizeof(ms_afCameraDir));
}

void CSpeedTreeRT::SetCamera(const float* pPosition, const float* pDirection)
{
    if (pPosition) std::memcpy(ms_afCameraPos, pPosition, sizeof(ms_afCameraPos));
    if (pDirection) std::memcpy(ms_afCameraDir, pDirection, sizeof(ms_afCameraDir));
}

void CSpeedTreeRT::SetTime(float fTime) {}
void CSpeedTreeRT::ComputeWindEffects(bool bBranches, bool bLeaves, bool bFronds) {}
void CSpeedTreeRT::ResetLeafWindState(void) {}
bool CSpeedTreeRT::GetLeafRockingState(void) const { return true; }
void CSpeedTreeRT::SetLeafRockingState(bool bFlag) {}
void CSpeedTreeRT::SetNumLeafRockingGroups(unsigned int nRockingGroups) {}

CSpeedTreeRT::EWindMethod CSpeedTreeRT::GetLeafWindMethod(void) const { return m_eLeafWind; }
void CSpeedTreeRT::SetLeafWindMethod(EWindMethod eMethod) { m_eLeafWind = eMethod; }
CSpeedTreeRT::EWindMethod CSpeedTreeRT::GetBranchWindMethod(void) const { return m_eBranchWind; }
void CSpeedTreeRT::SetBranchWindMethod(EWindMethod eMethod) { m_eBranchWind = eMethod; }
CSpeedTreeRT::EWindMethod CSpeedTreeRT::GetFrondWindMethod(void) const { return m_eFrondWind; }
void CSpeedTreeRT::SetFrondWindMethod(EWindMethod eMethod) { m_eFrondWind = eMethod; }

float CSpeedTreeRT::GetWindStrength(void) const { return m_fWindStrength; }
float CSpeedTreeRT::SetWindStrength(float fNewStrength, float fOldStrength, float fFrequencyTimeOffset)
{
    m_fWindStrength = fNewStrength;
    return m_fWindStrength;
}

void CSpeedTreeRT::SetNumWindMatrices(unsigned int nNumMatrices) { ms_unNumWindMatrices = nNumMatrices; }
void CSpeedTreeRT::SetWindMatrix(unsigned int nMatrixIndex, const float* pMatrix) {}
void CSpeedTreeRT::GetLocalMatrices(unsigned int& nStartingIndex, unsigned int& nMatrixSpan)
{
    nStartingIndex = 0;
    nMatrixSpan = 0;
}
void CSpeedTreeRT::SetLocalMatrices(unsigned int nStartingMatrix, unsigned int nMatrixSpan) {}

void CSpeedTreeRT::ComputeLodLevel(void) {}
float CSpeedTreeRT::GetLodLevel(void) const { return 1.0f; }
void CSpeedTreeRT::SetLodLevel(float fLodLevel) {}
void CSpeedTreeRT::SetDropToBillboard(bool bFlag) { ms_bDropToBillboard = bFlag; }

void CSpeedTreeRT::GetLodLimits(float& fNear, float& fFar) const { fNear = 100.0f; fFar = 1000.0f; }
void CSpeedTreeRT::SetLodLimits(float fNear, float fFar) {}

short CSpeedTreeRT::GetDiscreteBranchLodLevel(float fLodLevel) const { return 0; }
unsigned short CSpeedTreeRT::GetDiscreteLeafLodLevel(float fLodLevel) const { return 0; }
short CSpeedTreeRT::GetDiscreteFrondLodLevel(float fLodLevel) const { return 0; }

unsigned short CSpeedTreeRT::GetNumBranchLodLevels(void) const { return 1; }
unsigned short CSpeedTreeRT::GetNumLeafLodLevels(void) const { return 1; }
unsigned short CSpeedTreeRT::GetNumFrondLodLevels(void) const { return 1; }

void CSpeedTreeRT::DeleteBranchGeometry(void) {}
void CSpeedTreeRT::DeleteFrondGeometry(void) {}
unsigned char* CSpeedTreeRT::GetFrondGeometryMapIndexes(int nLodLevel) const { return nullptr; }
const float* CSpeedTreeRT::GetLeafBillboardTable(unsigned int& nEntryCount) const { nEntryCount = 0; return nullptr; }
const float* CSpeedTreeRT::GetLeafLodSizeAdjustments(void) { return nullptr; }

void CSpeedTreeRT::GenerateGeometry()
{
    if (m_bGeometryGenerated)
        return;
    m_bGeometryGenerated = true;

    const float H = (m_fSize > 1.0f) ? m_fSize : 400.0f;

    m_vBranchCoords.clear();
    m_vBranchColors.clear();
    m_vBranchTexCoords0.clear();
    m_vBranchTexCoords1.clear();
    m_vBranchStrip.clear();

    m_vLeafCenters.clear();
    m_vLeafColors.clear();
    m_vLeafSize.clear();
    m_vLeafHalfW.clear();
    m_vLeafHalfH.clear();
    m_vLeafAngle.clear();
    m_vLeafCoords.clear();
    m_vLeafTexCoords.clear();
    m_vLeafMapIndices.clear();
    m_vLeafClusterIndices.clear();

    // ------------------------------------------------------------------
    // Branches: tapered tubes stitched into a single triangle strip.
    // u wraps once around the tube, v tiles along its length.
    // ------------------------------------------------------------------
    auto addTube = [&](float sx, float sy, float sz,
                       float ex, float ey, float ez,
                       float r0, float r1, float vTile, bool bFlip)
    {
        float dx = ex - sx, dy = ey - sy, dz = ez - sz;
        float len = std::sqrt(dx * dx + dy * dy + dz * dz);
        if (len < 1e-4f)
            return;
        dx /= len; dy /= len; dz /= len;

        float rx = 0.0f, ry = 0.0f, rz = 1.0f;
        if (std::fabs(dz) > 0.9f) { rx = 1.0f; rz = 0.0f; }

        float rght[3] = { ry * dz - rz * dy, rz * dx - rx * dz, rx * dy - ry * dx };
        float rl = std::sqrt(rght[0] * rght[0] + rght[1] * rght[1] + rght[2] * rght[2]);
        if (rl < 1e-6f) return;
        rght[0] /= rl; rght[1] /= rl; rght[2] /= rl;

        float up[3] = { dy * rght[2] - dz * rght[1],
                        dz * rght[0] - dx * rght[2],
                        dx * rght[1] - dy * rght[0] };

        const int sides = 6;
        const unsigned short base = static_cast<unsigned short>(m_vBranchCoords.size() / 3);
        const float sign = bFlip ? 1.0f : -1.0f;

        for (int i = 0; i <= sides; ++i) {
            const float a = sign * 2.0f * 3.14159265358979f * static_cast<float>(i) / static_cast<float>(sides);
            const float ca = std::cos(a), sa = std::sin(a);
            const float ox = rght[0] * ca + up[0] * sa;
            const float oy = rght[1] * ca + up[1] * sa;
            const float oz = rght[2] * ca + up[2] * sa;
            const float u = static_cast<float>(i) / static_cast<float>(sides);

            m_vBranchCoords.push_back(sx + ox * r0);
            m_vBranchCoords.push_back(sy + oy * r0);
            m_vBranchCoords.push_back(sz + oz * r0);
            m_vBranchColors.push_back(0xFFFFFFFFul);
            m_vBranchTexCoords0.push_back(u);
            m_vBranchTexCoords0.push_back(0.0f);
            m_vBranchTexCoords1.push_back(u);
            m_vBranchTexCoords1.push_back(0.0f);

            m_vBranchCoords.push_back(ex + ox * r1);
            m_vBranchCoords.push_back(ey + oy * r1);
            m_vBranchCoords.push_back(ez + oz * r1);
            m_vBranchColors.push_back(0xFFFFFFFFul);
            m_vBranchTexCoords0.push_back(u);
            m_vBranchTexCoords0.push_back(vTile);
            m_vBranchTexCoords1.push_back(u);
            m_vBranchTexCoords1.push_back(vTile);
        }

        const unsigned short endIdx = base + static_cast<unsigned short>(2 * (sides + 1));
        if (m_vBranchStrip.empty()) {
            for (unsigned short k = base; k < endIdx; ++k)
                m_vBranchStrip.push_back(k);
        } else {
            const unsigned short last = m_vBranchStrip.back();
            m_vBranchStrip.push_back(last);   // degenerate stitch
            m_vBranchStrip.push_back(base);
            m_vBranchStrip.push_back(base);
            for (unsigned short k = static_cast<unsigned short>(base + 1); k < endIdx; ++k)
                m_vBranchStrip.push_back(k);
        }
    };

    // Double sided tubes: the branch pass uses backface culling.
    auto addTaperedTube = [&](float sx, float sy, float sz,
                              float ex, float ey, float ez,
                              float r0, float r1, float vTile)
    {
        addTube(sx, sy, sz, ex, ey, ez, r0, r1, vTile, false);
        addTube(sx, sy, sz, ex, ey, ez, r0, r1, vTile, true);
    };

    // Deterministic pseudo-random generator (same topology for every instance).
    unsigned int seed = 0x9E3779B9u ^ m_nSeed ^
                        (static_cast<unsigned int>(m_afPos[0] * 13.0f) * 2654435761u) ^
                        (static_cast<unsigned int>(m_afPos[1] * 7.0f) * 40503u);
    auto rnd = [&seed]() -> float {
        seed = seed * 1664525u + 1013904223u;
        return static_cast<float>((seed >> 8) & 0xFFFFFF) / 16777216.0f;
    };

    // Leaf-cluster cells of the composite atlas (left half of every shipped
    // atlas, rows 2-4 contain leaf clusters).
    static const float kUvRects[6][4] = {
        { 0.00f, 0.25f, 0.25f, 0.50f },
        { 0.25f, 0.25f, 0.50f, 0.50f },
        { 0.00f, 0.50f, 0.25f, 0.75f },
        { 0.25f, 0.50f, 0.50f, 0.75f },
        { 0.00f, 0.75f, 0.25f, 1.00f },
        { 0.25f, 0.75f, 0.50f, 1.00f },
    };

    int nLeafIndex = 0;
    auto addLeaf = [&](float cx, float cy, float cz, float size) {
        m_vLeafCenters.push_back(cx);
        m_vLeafCenters.push_back(cy);
        m_vLeafCenters.push_back(cz);
        m_vLeafColors.push_back(0xFFFFFFFFul);
        m_vLeafSize.push_back(size);
        m_vLeafHalfW.push_back(size * 0.5f);
        m_vLeafHalfH.push_back(size * 0.5f);
        m_vLeafAngle.push_back(rnd() * 6.2831853f);

        std::array<float, 16> coords{};
        coords[3] = coords[7] = coords[11] = coords[15] = 0.0f;
        m_vLeafCoords.push_back(coords);

        const float* uv = kUvRects[nLeafIndex % 6];
        std::array<float, 8> tex{};
        tex[0] = uv[0]; tex[1] = uv[3]; // bottom left
        tex[2] = uv[2]; tex[3] = uv[3]; // bottom right
        tex[4] = uv[2]; tex[5] = uv[1]; // top right
        tex[6] = uv[0]; tex[7] = uv[1]; // top left
        m_vLeafTexCoords.push_back(tex);

        m_vLeafMapIndices.push_back(0);
        m_vLeafClusterIndices.push_back(static_cast<unsigned char>(nLeafIndex % 4));
        ++nLeafIndex;
    };

    switch (m_eShape)
    {
    case SHAPE_CONIFER:
    {
        addTaperedTube(0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.94f * H,
                       0.030f * H, 0.004f * H, 0.94f * H / 250.0f);

        // Branch whorls, longer at the bottom.
        const int nTiers = 8;
        for (int t = 0; t < nTiers; ++t) {
            const float z = (0.22f + 0.09f * static_cast<float>(t)) * H;
            const float f = 1.0f - 0.72f * (z / H);
            const float len = 0.30f * H * f;
            const int nPerTier = 5;
            for (int b = 0; b < nPerTier; ++b) {
                const float a = 2.0f * 3.14159265358979f * static_cast<float>(b) / static_cast<float>(nPerTier)
                              + 0.7f * static_cast<float>(t);
                float dx = std::cos(a), dy = std::sin(a), dz = -0.40f;
                const float dl = std::sqrt(dx * dx + dy * dy + dz * dz);
                dx /= dl; dy /= dl; dz /= dl;
                addTaperedTube(0.0f, 0.0f, z, dx * len, dy * len, z + dz * len,
                               0.006f * H, 0.0015f * H, 0.22f * H / 250.0f);
            }
        }

        // Cone of needle cards.
        for (int i = 0; i < 150; ++i) {
            const float t = rnd();
            const float z = (0.24f + 0.72f * t) * H;
            const float rad = 0.32f * H * (1.0f - t) * (0.70f + 0.55f * rnd());
            const float a = rnd() * 6.2831853f;
            addLeaf(std::cos(a) * rad, std::sin(a) * rad, z, (0.09f + 0.05f * rnd()) * H);
        }
        break;
    }

    case SHAPE_PALM:
    {
        addTaperedTube(0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.86f * H,
                       0.034f * H, 0.018f * H, 0.86f * H / 250.0f);

        // Crown of drooping fronds made of leaf cards along arcs.
        const int nFronds = 9;
        for (int f = 0; f < nFronds; ++f) {
            const float a = 2.0f * 3.14159265358979f * static_cast<float>(f) / static_cast<float>(nFronds) + 0.3f;
            for (int k = 0; k < 4; ++k) {
                const float t = 0.18f + 0.24f * static_cast<float>(k);
                const float rad = t * 0.46f * H;
                const float z = 0.86f * H + 0.10f * H * (1.0f - t) - 0.22f * H * t * t;
                addLeaf(std::cos(a) * rad, std::sin(a) * rad, z, (0.22f - 0.03f * static_cast<float>(k)) * H);
            }
        }
        for (int i = 0; i < 8; ++i)
            addLeaf((rnd() - 0.5f) * 0.14f * H, (rnd() - 0.5f) * 0.14f * H,
                    0.86f * H + 0.05f * H * rnd(), 0.16f * H);
        break;
    }

    case SHAPE_BAOBAB:
    {
        addTaperedTube(0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.34f * H,
                       0.11f * H, 0.05f * H, 0.60f * H / 250.0f);

        for (int i = 0; i < 5; ++i) {
            const float a = 2.0f * 3.14159265358979f * static_cast<float>(i) / 5.0f + 0.4f;
            const float h0 = (0.28f + 0.02f * static_cast<float>(i)) * H;
            float dx = std::cos(a) * 0.80f, dy = std::sin(a) * 0.80f, dz = 0.60f;
            const float dl = std::sqrt(dx * dx + dy * dy + dz * dz);
            dx /= dl; dy /= dl; dz /= dl;
            const float len = 0.32f * H;
            addTaperedTube(0.0f, 0.0f, h0, dx * len, dy * len, h0 + dz * len,
                           0.030f * H, 0.008f * H, 0.30f * H / 250.0f);
        }

        for (int i = 0; i < 70; ++i) {
            const float a = rnd() * 6.2831853f;
            const float zc = rnd() * 2.0f - 1.0f;
            const float rr = std::sqrt(std::max(0.0f, 1.0f - zc * zc));
            addLeaf(0.38f * H * rr * std::cos(a),
                    0.38f * H * rr * std::sin(a),
                    0.62f * H + 0.22f * H * zc,
                    (0.18f + 0.08f * rnd()) * H);
        }
        break;
    }

    case SHAPE_SMALL:
    {
        addTaperedTube(0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.10f * H,
                       0.012f * H, 0.006f * H, 0.10f * H / 250.0f);

        for (int i = 0; i < 30; ++i) {
            const float a = 2.0f * 3.14159265358979f * static_cast<float>(i) / 30.0f + rnd() * 0.25f;
            const float rad = (0.10f + 0.24f * rnd()) * H;
            const float z = (0.08f + 0.38f * rnd()) * H;
            addLeaf(std::cos(a) * rad, std::sin(a) * rad, z, (0.18f + 0.12f * rnd()) * H);
        }
        break;
    }

    case SHAPE_BIRCH:
    {
        addTaperedTube(0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.60f * H,
                       0.026f * H, 0.010f * H, 0.60f * H / 250.0f);

        for (int i = 0; i < 4; ++i) {
            const float a = 1.5707963f * static_cast<float>(i) + 0.5f;
            const float h0 = (0.42f + 0.04f * static_cast<float>(i)) * H;
            float dx = std::cos(a) * 0.55f, dy = std::sin(a) * 0.55f, dz = 0.85f;
            const float dl = std::sqrt(dx * dx + dy * dy + dz * dz);
            dx /= dl; dy /= dl; dz /= dl;
            const float len = 0.24f * H;
            addTaperedTube(0.0f, 0.0f, h0, dx * len, dy * len, h0 + dz * len,
                           0.010f * H, 0.004f * H, 0.24f * H / 250.0f);
        }

        for (int i = 0; i < 84; ++i) {
            const float a = rnd() * 6.2831853f;
            const float zc = rnd() * 2.0f - 1.0f;
            const float rr = std::sqrt(std::max(0.0f, 1.0f - zc * zc));
            addLeaf(0.25f * H * rr * std::cos(a),
                    0.25f * H * rr * std::sin(a),
                    0.72f * H + 0.26f * H * zc,
                    (0.12f + 0.06f * rnd()) * H);
        }
        break;
    }

    case SHAPE_BROADLEAF:
    default:
    {
        addTaperedTube(0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.52f * H,
                       0.040f * H, 0.020f * H, 0.52f * H / 250.0f);

        float limbTips[5][3];
        for (int i = 0; i < 5; ++i) {
            const float a = 2.0f * 3.14159265358979f * static_cast<float>(i) / 5.0f + 0.35f;
            const float h0 = (0.36f + 0.035f * static_cast<float>(i)) * H;
            float dx = std::cos(a) * 0.62f, dy = std::sin(a) * 0.62f, dz = 0.78f;
            const float dl = std::sqrt(dx * dx + dy * dy + dz * dz);
            dx /= dl; dy /= dl; dz /= dl;
            const float len = 0.32f * H;
            limbTips[i][0] = dx * len;
            limbTips[i][1] = dy * len;
            limbTips[i][2] = h0 + dz * len;
            addTaperedTube(0.0f, 0.0f, h0, limbTips[i][0], limbTips[i][1], limbTips[i][2],
                           0.016f * H, 0.005f * H, 0.32f * H / 250.0f);
        }

        for (int i = 0; i < 10; ++i) {
            const int li = i % 5;
            const float sx = limbTips[li][0] * 0.6f;
            const float sy = limbTips[li][1] * 0.6f;
            const float sz = limbTips[li][2] - 0.04f * H;
            const float a = 1.2566371f * static_cast<float>(i) + 0.9f;
            float dx = std::cos(a) * 0.9f, dy = std::sin(a) * 0.9f, dz = 0.5f;
            const float dl = std::sqrt(dx * dx + dy * dy + dz * dz);
            dx /= dl; dy /= dl; dz /= dl;
            const float len = 0.18f * H;
            addTaperedTube(sx, sy, sz, sx + dx * len, sy + dy * len, sz + dz * len,
                           0.006f * H, 0.002f * H, 0.18f * H / 250.0f);
        }

        for (int i = 0; i < 96; ++i) {
            const float a = rnd() * 6.2831853f;
            const float zc = rnd() * 2.0f - 1.0f;
            const float rr = std::sqrt(std::max(0.0f, 1.0f - zc * zc));
            addLeaf(0.32f * H * rr * std::cos(a),
                    0.32f * H * rr * std::sin(a),
                    0.70f * H + 0.24f * H * zc,
                    (0.15f + 0.09f * rnd()) * H);
        }
        break;
    }
    }

    m_usBranchStripLength = static_cast<unsigned short>(m_vBranchStrip.size());
    m_vpBranchStrips.clear();
    m_vpBranchStrips.push_back(m_vBranchStrip.data());

    m_vpLeafCoords.clear();
    m_vpLeafTexCoords.clear();
    for (size_t i = 0; i < m_vLeafCoords.size(); ++i) {
        m_vpLeafCoords.push_back(m_vLeafCoords[i].data());
        m_vpLeafTexCoords.push_back(m_vLeafTexCoords[i].data());
    }

    UpdateLeafBillboards();

    // bounds + trunk collision
    m_afBoundingBox[0] = -0.40f * H;
    m_afBoundingBox[1] = -0.40f * H;
    m_afBoundingBox[2] = 0.0f;
    m_afBoundingBox[3] = 0.40f * H;
    m_afBoundingBox[4] = 0.40f * H;
    m_afBoundingBox[5] = 1.0f * H;

    m_vCollisions.clear();
    SCollisionObj obj;
    obj.m_eType = CO_CYLINDER;
    obj.m_afPos[0] = 0.0f;
    obj.m_afPos[1] = 0.0f;
    obj.m_afPos[2] = 0.0f;
    obj.m_afDims[0] = (m_eShape == SHAPE_BAOBAB) ? 0.10f * H : 0.045f * H;
    obj.m_afDims[1] = 0.60f * H;
    obj.m_afDims[2] = 0.0f;
    m_vCollisions.push_back(obj);
}

namespace {

struct SBakedReader
{
    const unsigned char* m_pData;
    size_t m_nSize;
    size_t m_nPos;

    SBakedReader(const unsigned char* p, size_t n) : m_pData(p), m_nSize(n), m_nPos(0) {}

    bool Valid(size_t n) const { return m_nPos + n <= m_nSize; }
    size_t Remaining() const { return m_nPos <= m_nSize ? m_nSize - m_nPos : 0; }
    bool Skip(size_t n) { if (!Valid(n)) return false; m_nPos += n; return true; }
    unsigned char U8() { unsigned char v = Valid(1) ? m_pData[m_nPos] : 0; m_nPos += 1; return v; }
    unsigned short U16() { unsigned short v = 0; if (Valid(2)) std::memcpy(&v, m_pData + m_nPos, 2); m_nPos += 2; return v; }
    uint32_t U32() { uint32_t v = 0; if (Valid(4)) std::memcpy(&v, m_pData + m_nPos, 4); m_nPos += 4; return v; }
    int32_t I32() { int32_t v = 0; if (Valid(4)) std::memcpy(&v, m_pData + m_nPos, 4); m_nPos += 4; return v; }
    float F32() { float v = 0.0f; if (Valid(4)) std::memcpy(&v, m_pData + m_nPos, 4); m_nPos += 4; return v; }
    void Pluck(const unsigned char*& p, size_t n) { p = Valid(n) ? m_pData + m_nPos : nullptr; m_nPos += n; }
};

// Walks the same layout as LoadBakedGeometry without touching any member state.
// Rejecting a malformed cache here is important: a failed load must not leave a
// partially parsed bounding box or collision list behind, because the caller
// falls back to generated geometry.
static bool ValidateBakedGeometry(SBakedReader & r, unsigned int nHash)
{
    if (r.U32() != TreeCache::kMagic || r.U32() != nHash)
        return false;
    if (r.Remaining() < 28)
        return false;

    r.Skip(24); // bbox

    const uint32_t nColl = r.U32();
    if (nColl > r.Remaining() / 28)
        return false;
    if (!r.Skip(static_cast<size_t>(nColl) * 28))
        return false;

    for (int nPart = 0; nPart < 2; ++nPart) {
        const uint32_t nVertexCount = r.U32();
        const uint32_t nNumStrips = r.U32();
        r.U16(); // lod
        r.F32(); // alpha test
        const uint32_t nIndexCount = r.U32();

        if (nVertexCount > 0xFFFF || nNumStrips > 0xFFFF)
            return false;
        if (nIndexCount > 0 && nNumStrips == 0)
            return false;

        const size_t nBytes = static_cast<size_t>(nIndexCount) * 2 +
                              static_cast<size_t>(nVertexCount) * 44;
        if (!r.Skip(nBytes))
            return false;
    }

    for (int nLeafSet = 0; nLeafSet < 2; ++nLeafSet) {
        r.U8();     // active
        r.I32();    // lod
        r.F32();    // alpha test
        const uint32_t nCount = r.U32();
        if (nCount > 0xFFFF)
            return false;
        if (!r.Skip(static_cast<size_t>(nCount) * 114))
            return false;
    }

    return true;
}

} // namespace

bool CSpeedTreeRT::LoadBakedGeometry(unsigned int nHash)
{
    char szPath[512];
    static const char* s_aszDirs[] = { "treecache/", "../treecache/", "dist/treecache/" };

    std::ifstream file;
    for (size_t i = 0; i < sizeof(s_aszDirs) / sizeof(s_aszDirs[0]) && !file.is_open(); ++i) {
        std::snprintf(szPath, sizeof(szPath), "%s%08x.tree", s_aszDirs[i], nHash);
        file.open(szPath, std::ios::binary);
    }
    if (!file.is_open()) {
        const char* pEnv = std::getenv("MT2_TREECACHE");
        if (pEnv && pEnv[0]) {
            std::snprintf(szPath, sizeof(szPath), "%s/%08x.tree", pEnv, nHash);
            file.open(szPath, std::ios::binary);
        }
    }
    if (!file.is_open())
        return false;

    file.seekg(0, std::ios::end);
    const std::streamoff nSize = file.tellg();
    file.seekg(0, std::ios::beg);
    if (nSize <= 0) return false;

    std::vector<unsigned char> data(static_cast<size_t>(nSize));
    file.read(reinterpret_cast<char*>(data.data()), nSize);
    file.close();

    SBakedReader validator(data.data(), data.size());
    if (!ValidateBakedGeometry(validator, nHash))
        return false;

    SBakedReader r(data.data(), data.size());
    if (r.U32() != TreeCache::kMagic || r.U32() != nHash)
        return false;

    float fBBox[6];
    for (int i = 0; i < 6; ++i) fBBox[i] = r.F32();
    std::memcpy(m_afBoundingBox, fBBox, sizeof(m_afBoundingBox));

    m_vCollisions.clear();
    const uint32_t nColl = r.U32();
    // Every collision record is i32 + 6 floats. Reject a count the file cannot
    // possibly contain before allocating or looping.
    if (nColl > r.Remaining() / 28)
        return false;
    for (uint32_t i = 0; i < nColl; ++i) {
        SCollisionObj obj;
        obj.m_eType = (ECollisionObjectType)r.I32();
        obj.m_afPos[0] = r.F32(); obj.m_afPos[1] = r.F32(); obj.m_afPos[2] = r.F32();
        obj.m_afDims[0] = r.F32(); obj.m_afDims[1] = r.F32(); obj.m_afDims[2] = r.F32();
        m_vCollisions.push_back(obj);
    }

    m_vBranchCoords.clear(); m_vBranchColors.clear();
    m_vBranchTexCoords0.clear(); m_vBranchTexCoords1.clear();
    m_vBranchStrip.clear(); m_vpBranchStrips.clear();
    m_usBranchStripLength = 0;

    m_vFrondCoords.clear(); m_vFrondColors.clear();
    m_vFrondTexCoords0.clear(); m_vFrondTexCoords1.clear();
    m_vFrondStrip.clear(); m_vpFrondStrips.clear();
    m_usFrondStripLength = 0;

    m_vLeafCenters.clear(); m_vLeafColors.clear();
    m_vLeafSize.clear(); m_vLeafHalfW.clear(); m_vLeafHalfH.clear();
    m_vLeafAngle.clear(); m_vLeafCoords.clear(); m_vLeafTexCoords.clear();
    m_vLeafMapIndices.clear(); m_vLeafClusterIndices.clear();

    for (int nPart = 0; nPart < 2; ++nPart) {
        const uint32_t nVertexRaw = r.U32();
        const uint32_t nNumStrips = r.U32();
        r.U16(); // lod
        const float fAlpha = r.F32();
        const uint32_t nIndexCount = r.U32();

        if (nVertexRaw > 0xFFFF)
            return false;
        if (nNumStrips > 0xFFFF || (nIndexCount > 0 && nNumStrips == 0))
            return false;

        // u16 indices plus 44 bytes per vertex (12 coords + 4 colour + 16 UV +
        // 12 normals).
        const size_t nVertexBytes = static_cast<size_t>(nVertexRaw) * 44;
        const size_t nIndexBytes = static_cast<size_t>(nIndexCount) * 2;
        if (nIndexBytes + nVertexBytes > r.Remaining())
            return false;

        const unsigned short nVertexCount = static_cast<unsigned short>(nVertexRaw);

        // Read the strips. The library composites branch geometry into a
        // single strip (Compute(..., bCompositeStrips=true)); fronds may have
        // more than one, so stitch them with degenerate indices.
        std::vector<unsigned short> indices;
        indices.reserve(nIndexCount + 8);
        const uint32_t nPerStrip = (nNumStrips > 0) ? (nIndexCount / nNumStrips) : 0;
        for (uint32_t s = 0; s < nNumStrips; ++s) {
            const uint32_t nCount = (s + 1 == nNumStrips) ? (nIndexCount - s * nPerStrip) : nPerStrip;
            std::vector<unsigned short> stripIndices(nCount);
            for (uint32_t k = 0; k < nCount; ++k) stripIndices[k] = r.U16();
            if (nCount == 0) continue;
            if (indices.empty()) {
                indices = stripIndices;
            } else {
                indices.push_back(indices.back());
                indices.push_back(stripIndices[0]);
                indices.push_back(stripIndices[0]);
                indices.insert(indices.end(), stripIndices.begin() + 1, stripIndices.end());
            }
        }

        std::vector<float> coords(nVertexCount * 3);
        for (size_t i = 0; i < coords.size(); ++i) coords[i] = r.F32();
        std::vector<unsigned long> colors(nVertexCount);
        for (unsigned short i = 0; i < nVertexCount; ++i) colors[i] = r.U32();
        std::vector<float> tex0(nVertexCount * 2);
        for (size_t i = 0; i < tex0.size(); ++i) tex0[i] = r.F32();
        std::vector<float> tex1(nVertexCount * 2);
        for (size_t i = 0; i < tex1.size(); ++i) tex1[i] = r.F32();
        for (unsigned short i = 0; i < nVertexCount * 3; ++i) (void)r.F32(); // normals

        if (nVertexCount == 0)
            continue;

        if (nPart == 0) {
            m_vBranchCoords.swap(coords);
            m_vBranchColors.swap(colors);
            m_vBranchTexCoords0.swap(tex0);
            m_vBranchTexCoords1.swap(tex1);
            m_vBranchStrip.swap(indices);
            m_fBranchAlpha = fAlpha;
        } else {
            m_vFrondCoords.swap(coords);
            m_vFrondColors.swap(colors);
            m_vFrondTexCoords0.swap(tex0);
            m_vFrondTexCoords1.swap(tex1);
            m_vFrondStrip.swap(indices);
            m_fFrondAlpha = fAlpha;
        }
    }

    m_vpBranchStrips.push_back(m_vBranchStrip.data());
    m_usBranchStripLength = (unsigned short)m_vBranchStrip.size();
    m_vpFrondStrips.push_back(m_vFrondStrip.data());
    m_usFrondStripLength = (unsigned short)m_vFrondStrip.size();

    for (int nLeafSet = 0; nLeafSet < 2; ++nLeafSet) {
        const bool bActive = r.U8() != 0;
        r.I32(); // lod
        const float fAlpha = r.F32();
        const uint32_t nCount = r.U32();

        // 114 bytes per leaf: 3 centre floats, colour, 16 corner floats,
        // 8 UV floats and the two one-byte indices.
        if (nCount > 0xFFFF || static_cast<size_t>(nCount) * 114 > r.Remaining())
            return false;

        std::vector<float> centers(nCount * 3);
        std::vector<unsigned long> colors(nCount);
        for (uint32_t i = 0; i < nCount * 3; ++i) centers[i] = r.F32();
        for (uint32_t i = 0; i < nCount; ++i) colors[i] = r.U32();

        std::vector<std::array<float, 16> > coords(nCount);
        std::vector<std::array<float, 8> > texCoords(nCount);
        std::vector<unsigned char> mapIdx(nCount), clusterIdx(nCount);
        for (uint32_t i = 0; i < nCount; ++i) {
            for (int k = 0; k < 16; ++k) coords[i][k] = r.F32();
            for (int k = 0; k < 8; ++k) texCoords[i][k] = r.F32();
            mapIdx[i] = r.U8();
            clusterIdx[i] = r.U8();
        }

        if (nLeafSet != 0 || !bActive || nCount == 0)
            continue;

        m_vLeafCenters = centers;
        m_vLeafColors = colors;
        m_vLeafCoords = coords;
        m_vLeafTexCoords = texCoords;
        m_vLeafMapIndices = mapIdx;
        m_vLeafClusterIndices = clusterIdx;
        m_vLeafHalfW.resize(nCount);
        m_vLeafHalfH.resize(nCount);
        m_vLeafSize.resize(nCount);
        m_vLeafAngle.resize(nCount);

        for (uint32_t i = 0; i < nCount; ++i) {
            const std::array<float, 16>& c = coords[i];
            const float dx = c[4] - c[0], dy = c[5] - c[1], dz = c[6] - c[2];
            const float ux = c[12] - c[0], uy = c[13] - c[1], uz = c[14] - c[2];
            const float fW = std::sqrt(dx * dx + dy * dy + dz * dz);
            const float fH = std::sqrt(ux * ux + uy * uy + uz * uz);
            m_vLeafHalfW[i] = fW * 0.5f;
            m_vLeafHalfH[i] = fH * 0.5f;
            m_vLeafSize[i] = std::max(fW, fH);
            m_vLeafAngle[i] = std::atan2(dz, dx);
        }
        m_fLeafAlpha = fAlpha;
    }

    m_vpLeafCoords.clear();
    m_vpLeafTexCoords.clear();
    for (size_t i = 0; i < m_vLeafCoords.size(); ++i) {
        m_vpLeafCoords.push_back(m_vLeafCoords[i].data());
        m_vpLeafTexCoords.push_back(m_vLeafTexCoords[i].data());
    }

    m_bBakedGeometry = true;
    m_bGeometryGenerated = true;
    return true;
}

void CSpeedTreeRT::UpdateLeafBillboards()
{
    UpdateLeafBillboardsAt(m_afPos);
}

void CSpeedTreeRT::UpdateLeafBillboardsAt(const float* pTreePos)
{
    const size_t nLeafCount = m_vLeafCenters.size() / 3;
    if (nLeafCount == 0 || m_vLeafCoords.size() != nLeafCount)
        return;

    // Camera facing basis around the tree position.
    float vx = pTreePos[0] - ms_afCameraPos[0];
    float vy = pTreePos[1] - ms_afCameraPos[1];
    float vz = pTreePos[2] - ms_afCameraPos[2];
    float vl = std::sqrt(vx * vx + vy * vy + vz * vz);
    if (vl < 1e-4f) { vx = 0.0f; vy = 1.0f; vz = 0.0f; }
    else { vx /= vl; vy /= vl; vz /= vl; }

    // right = normalize(cross(worldUp, view))
    float rght[3] = { 0.0f * vz - 1.0f * vy, 1.0f * vx - 0.0f * vz, 0.0f * vy - 0.0f * vx };
    float rl = std::sqrt(rght[0] * rght[0] + rght[1] * rght[1] + rght[2] * rght[2]);
    if (rl < 1e-4f) { rght[0] = 1.0f; rght[1] = 0.0f; rght[2] = 0.0f; rl = 1.0f; }
    rght[0] /= rl; rght[1] /= rl; rght[2] /= rl;

    // up = cross(view, right)
    const float up[3] = { vy * rght[2] - vz * rght[1],
                          vz * rght[0] - vx * rght[2],
                          vx * rght[1] - vy * rght[0] };

    for (size_t i = 0; i < nLeafCount; ++i) {
        const float ca = std::cos(m_vLeafAngle[i]);
        const float sa = std::sin(m_vLeafAngle[i]);
        const float rx = rght[0] * ca + up[0] * sa;
        const float ry = rght[1] * ca + up[1] * sa;
        const float rz = rght[2] * ca + up[2] * sa;
        const float ux = up[0] * ca - rght[0] * sa;
        const float uy = up[1] * ca - rght[1] * sa;
        const float uz = up[2] * ca - rght[2] * sa;

        const float hw = (i < m_vLeafHalfW.size()) ? m_vLeafHalfW[i] : (m_vLeafSize[i] * 0.5f);
        const float hh = (i < m_vLeafHalfH.size()) ? m_vLeafHalfH[i] : (m_vLeafSize[i] * 0.5f);

        std::array<float, 16>& c = m_vLeafCoords[i];
        // Baked corner order: 0 = top-left, 1 = top-right,
        //                     2 = bottom-right, 3 = bottom-left.
        c[0] = -rx * hw + ux * hh; c[1] = -ry * hw + uy * hh; c[2] = -rz * hw + uz * hh; c[3] = 0.0f;
        c[4] =  rx * hw + ux * hh; c[5] =  ry * hw + uy * hh; c[6] =  rz * hw + uz * hh; c[7] = 0.0f;
        c[8] =  rx * hw - ux * hh; c[9] =  ry * hw - uy * hh; c[10] = rz * hw - uz * hh; c[11] = 0.0f;
        c[12] = -rx * hw - ux * hh; c[13] = -ry * hw - uy * hh; c[14] = -rz * hw - uz * hh; c[15] = 0.0f;
    }
}

void CSpeedTreeRT::GetGeometry(SGeometry& sGeometry, unsigned long ulBitVector, short sOverrideBranchLodValue, short sOverrideFrondLodValue, short sOverrideLeafLodValue)
{
    (void)sOverrideBranchLodValue;
    (void)sOverrideFrondLodValue;
    (void)sOverrideLeafLodValue;

    // Instances share the geometry of their parent tree; only the leaf cards
    // are re-oriented for the instance position.
    if (m_pInstanceOf)
    {
        CSpeedTreeRT* pParent = const_cast<CSpeedTreeRT*>(m_pInstanceOf);
        pParent->GetGeometry(sGeometry, ulBitVector);
        if (ulBitVector & SpeedTree_LeafGeometry)
            pParent->UpdateLeafBillboardsAt(m_afPos);
        return;
    }

    if (!m_bGeometryGenerated)
        GenerateGeometry();

    if (ulBitVector & SpeedTree_BranchGeometry)
    {
        sGeometry.m_sBranches.m_usVertexCount = static_cast<unsigned short>(m_vBranchCoords.size() / 3);
        sGeometry.m_sBranches.m_nDiscreteLodLevel = 0;
        sGeometry.m_sBranches.m_usNumStrips = m_vBranchStrip.empty() ? 0 : 1;
        sGeometry.m_sBranches.m_pStripLengths = &m_usBranchStripLength;
        sGeometry.m_sBranches.m_pStrips = m_vpBranchStrips.data();
        sGeometry.m_sBranches.m_pColors = m_vBranchColors.data();
        sGeometry.m_sBranches.m_pCoords = m_vBranchCoords.data();
        sGeometry.m_sBranches.m_pTexCoords0 = m_vBranchTexCoords0.data();
        sGeometry.m_sBranches.m_pTexCoords1 = m_vBranchTexCoords1.data();
        sGeometry.m_sBranches.m_pNormals = nullptr;
        sGeometry.m_sBranches.m_pBinormals = nullptr;
        sGeometry.m_sBranches.m_pTangents = nullptr;
        sGeometry.m_sBranches.m_pWindWeights = nullptr;
        sGeometry.m_sBranches.m_pWindMatrixIndices = nullptr;
        sGeometry.m_fBranchAlphaTestValue = m_fBranchAlpha;
    }

    if (ulBitVector & SpeedTree_FrondGeometry)
    {
        if (m_vFrondCoords.empty())
        {
            sGeometry.m_sFronds.m_usVertexCount = 0;
            sGeometry.m_sFronds.m_nDiscreteLodLevel = -1;
            sGeometry.m_sFronds.m_usNumStrips = 0;
            sGeometry.m_fFrondAlphaTestValue = 0.0f;
        }
        else
        {
            sGeometry.m_sFronds.m_usVertexCount = static_cast<unsigned short>(m_vFrondCoords.size() / 3);
            sGeometry.m_sFronds.m_nDiscreteLodLevel = 0;
            sGeometry.m_sFronds.m_usNumStrips = 1;
            sGeometry.m_sFronds.m_pStripLengths = &m_usFrondStripLength;
            sGeometry.m_sFronds.m_pStrips = m_vpFrondStrips.data();
            sGeometry.m_sFronds.m_pColors = m_vFrondColors.data();
            sGeometry.m_sFronds.m_pCoords = m_vFrondCoords.data();
            sGeometry.m_sFronds.m_pTexCoords0 = m_vFrondTexCoords0.data();
            sGeometry.m_sFronds.m_pTexCoords1 = m_vFrondTexCoords1.data();
            sGeometry.m_sFronds.m_pNormals = nullptr;
            sGeometry.m_sFronds.m_pBinormals = nullptr;
            sGeometry.m_sFronds.m_pTangents = nullptr;
            sGeometry.m_sFronds.m_pWindWeights = nullptr;
            sGeometry.m_sFronds.m_pWindMatrixIndices = nullptr;
            sGeometry.m_fFrondAlphaTestValue = m_fFrondAlpha;
        }
    }

    if (ulBitVector & SpeedTree_LeafGeometry)
    {
        UpdateLeafBillboards();

        sGeometry.m_sLeaves0.m_bIsActive = !m_vLeafCenters.empty();
        sGeometry.m_sLeaves0.m_fAlphaTestValue = m_fLeafAlpha;
        sGeometry.m_sLeaves0.m_nDiscreteLodLevel = 0;
        sGeometry.m_sLeaves0.m_usLeafCount = static_cast<unsigned short>(m_vLeafCenters.size() / 3);
        sGeometry.m_sLeaves0.m_pLeafMapIndices = m_vLeafMapIndices.data();
        sGeometry.m_sLeaves0.m_pLeafClusterIndices = m_vLeafClusterIndices.data();
        sGeometry.m_sLeaves0.m_pCenterCoords = m_vLeafCenters.data();
        sGeometry.m_sLeaves0.m_pLeafMapTexCoords = m_vpLeafTexCoords.data();
        sGeometry.m_sLeaves0.m_pLeafMapCoords = m_vpLeafCoords.data();
        sGeometry.m_sLeaves0.m_pColors = m_vLeafColors.data();
        sGeometry.m_sLeaves0.m_pNormals = nullptr;
        sGeometry.m_sLeaves0.m_pBinormals = nullptr;
        sGeometry.m_sLeaves0.m_pTangents = nullptr;
        sGeometry.m_sLeaves0.m_pWindWeights = nullptr;
        sGeometry.m_sLeaves0.m_pWindMatrixIndices = nullptr;

        sGeometry.m_sLeaves1.m_bIsActive = false;
        sGeometry.m_sLeaves1.m_usLeafCount = 0;
        sGeometry.m_sLeaves1.m_nDiscreteLodLevel = -1;
    }

    if (ulBitVector & SpeedTree_BillboardGeometry)
    {
        sGeometry.m_sBillboard0.m_bIsActive = false;
        sGeometry.m_sBillboard1.m_bIsActive = false;
        sGeometry.m_sHorizontalBillboard.m_bIsActive = false;
    }
}

void CSpeedTreeRT::GetTextures(STextures& sTextures) const
{
    sTextures.m_pBranchTextureFilename = m_strBranchTexture.empty() ? nullptr : m_strBranchTexture.c_str();
    sTextures.m_pCompositeFilename = m_strCompositeTexture.empty() ? nullptr : m_strCompositeTexture.c_str();
    sTextures.m_pSelfShadowFilename = m_strSelfShadowTexture.empty() ? nullptr : m_strSelfShadowTexture.c_str();

    m_vpLeafTextures.clear();
    for (const auto& s : m_vLeafTextures)
        m_vpLeafTextures.push_back(s.c_str());
    sTextures.m_uiLeafTextureCount = static_cast<unsigned int>(m_vpLeafTextures.size());
    sTextures.m_pLeafTextureFilenames = m_vpLeafTextures.empty() ? nullptr : m_vpLeafTextures.data();

    m_vpFrondTextures.clear();
    for (const auto& s : m_vFrondTextures)
        m_vpFrondTextures.push_back(s.c_str());
    sTextures.m_uiFrondTextureCount = static_cast<unsigned int>(m_vpFrondTextures.size());
    sTextures.m_pFrondTextureFilenames = m_vpFrondTextures.empty() ? nullptr : m_vpFrondTextures.data();
}

void CSpeedTreeRT::SetLeafTextureCoords(unsigned int nLeafMapIndex, const float* pTexCoords) {}
void CSpeedTreeRT::SetFrondTextureCoords(unsigned int nFrondMapIndex, const float* pTexCoords) {}
bool CSpeedTreeRT::GetTextureFlip(void) { return ms_bTextureFlip; }
void CSpeedTreeRT::SetTextureFlip(bool bFlag) { ms_bTextureFlip = bFlag; }

void CSpeedTreeRT::SetBranchTextureFilename(const char* pFilename)
{
    m_strBranchTexture = pFilename ? pFilename : "";
}

void CSpeedTreeRT::SetLeafTextureFilename(unsigned int nLeafMapIndex, const char* pFilename)
{
    if (nLeafMapIndex < m_vLeafTextures.size() && pFilename)
        m_vLeafTextures[nLeafMapIndex] = pFilename;
}

void CSpeedTreeRT::SetFrondTextureFilename(unsigned int nFrondMapIndex, const char* pFilename)
{
    if (nFrondMapIndex < m_vFrondTextures.size() && pFilename)
        m_vFrondTextures[nFrondMapIndex] = pFilename;
}

void CSpeedTreeRT::Authorize(const char* pKey) {}
bool CSpeedTreeRT::IsAuthorized(void) { return true; }
const char* CSpeedTreeRT::GetCurrentError(void) { return ms_strError.c_str(); }
void CSpeedTreeRT::ResetError(void) { ms_strError.clear(); }

void CSpeedTreeRT::GetBoundingBox(float* pBounds) const
{
    if (pBounds)
        std::memcpy(pBounds, m_afBoundingBox, sizeof(m_afBoundingBox));
}

unsigned int CSpeedTreeRT::GetLeafTriangleCount(float fLodLevel) const { (void)fLodLevel; return (unsigned int)(m_vLeafCenters.size() / 3) * 2; }
unsigned int CSpeedTreeRT::GetBranchTriangleCount(float fLodLevel) const { (void)fLodLevel; return m_usBranchStripLength > 2 ? (unsigned int)(m_usBranchStripLength - 2) : 0; }
unsigned int CSpeedTreeRT::GetFrondTriangleCount(float fLodLevel) const { return 0; }

unsigned int CSpeedTreeRT::GetCollisionObjectCount(void)
{
    return static_cast<unsigned int>(m_vCollisions.size());
}

void CSpeedTreeRT::GetCollisionObject(unsigned int nIndex, ECollisionObjectType& eType, float* pPosition, float* pDimensions)
{
    if (nIndex < m_vCollisions.size())
    {
        eType = m_vCollisions[nIndex].m_eType;
        if (pPosition) std::memcpy(pPosition, m_vCollisions[nIndex].m_afPos, 3 * sizeof(float));
        if (pDimensions) std::memcpy(pDimensions, m_vCollisions[nIndex].m_afDims, 3 * sizeof(float));
    }
}

const char* CSpeedTreeRT::GetUserData(void) const
{
    return nullptr;
}
