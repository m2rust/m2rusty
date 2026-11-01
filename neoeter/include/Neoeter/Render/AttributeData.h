#pragma once

#include "Resource.h"
#include "Ref.h"
#include "CollisionData.h"

using THeightData = struct SHeightData
{
    char szName[32 + 1]{};
    std::vector<D3DXVECTOR3> v3VertexVector;
};

using THeightDataVector = std::vector<THeightData>;

class CAttributeData : public CResource
{
  public:
    using TRef = CRef<CAttributeData>;

    /*
    enum ECollisionType
    {
        COLLISION_TYPE_PLANE,
        COLLISION_TYPE_BOX,
        COLLISION_TYPE_SPHERE,
        COLLISION_TYPE_CYLINDER,
    };

    typedef struct SCollisionData
    {
        DWORD dwType;
        char szName[32+1];

        D3DXVECTOR3 v3Position;
        float fDimensions[3];
        D3DXQUATERNION quatRotation;
    } TCollisionData;*/

    static TType Type();

    CAttributeData(const char *c_szFileName);
    ~CAttributeData() override;

    // DWORD GetCollisionDataCount() const;
    // BOOL GetCollisionDataPointer(DWORD dwIndex, const TCollisionData **
    // c_ppCollisionData) const;
    const CStaticCollisionDataVector &GetCollisionDataVector() const;
    const THeightDataVector &GetHeightDataVector() const;

    size_t AddCollisionData(const CStaticCollisionData &collisionData); // return m_StaticCollisionDataVector.size();

    DWORD GetHeightDataCount() const;
    BOOL GetHeightDataPointer(DWORD dwIndex, const THeightData **c_ppHeightData) const;

    float GetMaximizeRadius() const;

  protected:
    bool OnLoad(int iSize, const void *c_pvBuf) override;
    void OnClear() override;
    bool OnIsEmpty() const override;
    bool OnIsType(TType type) override;
    void OnSelfDestruct() override;

    float m_fMaximizeRadius;

    // std::vector<TCollisionData> m_CollisionDataVector;
    CStaticCollisionDataVector m_StaticCollisionDataVector;
    THeightDataVector m_HeightDataVector;
};
