#include <utility>

// MonsterAreaInfo.h: interface for the CMonsterAreaInfo class.
//
//////////////////////////////////////////////////////////////////////

#ifndef AFX_MONSTERAREAINFO_H__B177E257_2EB2_4CE5_89D3_D06B5618BE1B__INCLUDED_
#define AFX_MONSTERAREAINFO_H__B177E257_2EB2_4CE5_89D3_D06B5618BE1B__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000

class CMonsterAreaInfo
{
  public:
    enum EMonsterAreaInfoType
    {
        MONSTERAREAINFOTYPE_INVALID,
        MONSTERAREAINFOTYPE_MONSTER,
        MONSTERAREAINFOTYPE_GROUP,
    };
    enum EMonsterDir
    {
        DIR_RANDOM = 0,
        DIR_NORTH,
        DIR_NORTHEAST,
        DIR_EAST,
        DIR_SOUTHEAST,
        DIR_SOUTH,
        DIR_SOUTHWEST,
        DIR_WEST,
        DIR_NORTHWEST
    };

    CMonsterAreaInfo();
    virtual ~CMonsterAreaInfo();

    void SetID(DWORD dwID)
    {
        m_dwID = dwID;
    }
    DWORD GetID() const
    {
        return m_dwID;
    }

    void Clear();

    //
    void SetOrigin(long lOriginX, long lOriginY);
    void GetOrigin(long *plOriginX, long *plOriginY) const;

    void SetSize(long lSizeX, long lSizeY);
    void GetSize(long *plSizeX, long *plSizeY) const;

    long GetLeft() const
    {
        return m_lLeft;
    }
    long GetTop() const
    {
        return m_lTop;
    }
    long GetRight() const
    {
        return m_lRight;
    }
    long GetBottom() const
    {
        return m_lBottom;
    }

    void SetMonsterAreaInfoType(EMonsterAreaInfoType eMonsterAreaInfoType)
    {
        m_eMonsterAreaInfoType = eMonsterAreaInfoType;
    }
    EMonsterAreaInfoType GetMonsterAreaInfoType()
    {
        return m_eMonsterAreaInfoType;
    }

    void SetMonsterGroupID(DWORD dwGroupID)
    {
        m_dwGroupID = dwGroupID;
    }
    void SetMonsterGroupName(std::string strGroupName)
    {
        m_strGroupName = std::move(strGroupName);
    }
    void SetMonsterGroupLeaderName(std::string strGroupLeaderName)
    {
        m_strLeaderName = std::move(strGroupLeaderName);
    }
    void SetMonsterGroupFollowerCount(DWORD dwFollowerCount)
    {
        m_dwFollowerCount = dwFollowerCount;
    }

    DWORD GetMonsterGroupID() const
    {
        return m_dwGroupID;
    }
    std::string GetMonsterGroupName()
    {
        return m_strGroupName;
    }
    std::string GetMonsterGroupLeaderName()
    {
        return m_strLeaderName;
    }
    DWORD GetMonsterGroupFollowerCount() const
    {
        return m_dwFollowerCount;
    }

    void SetMonsterName(std::string strMonsterName)
    {
        m_strMonsterName = std::move(strMonsterName);
    }
    void SetMonsterVID(DWORD dwVID)
    {
        m_dwVID = dwVID;
    }

    std::string GetMonsterName()
    {
        return m_strMonsterName;
    }
    DWORD GetMonsterVID() const
    {
        return m_dwVID;
    }

    void SetMonsterCount(DWORD dwCount);
    void SetMonsterDirection(EMonsterDir eMonsterDir);
    void RemoveAllMonsters();

    DWORD GetMonsterCount() const
    {
        return m_dwMonsterCount;
    }
    EMonsterDir GetMonsterDir()
    {
        return m_eMonsterDir;
    }
    D3DXVECTOR2 GetMonsterDirVector()
    {
        return m_v2Monsterdirection;
    }
    D3DXVECTOR2 GetTempMonsterPos(DWORD dwIndex);

  protected:
    void SetLRTB();

    EMonsterAreaInfoType m_eMonsterAreaInfoType;

    // Group Type 정보
    DWORD m_dwGroupID = 0U;
    std::string m_strGroupName;
    std::string m_strLeaderName;
    DWORD m_dwFollowerCount = 0U;

    // Monster Type 정보
    DWORD m_dwVID = 0U;
    std::string m_strMonsterName;

    // 공통 정보
    DWORD m_dwMonsterCount = 0U;
    EMonsterDir m_eMonsterDir;
    D3DXVECTOR2 m_v2Monsterdirection;

    DWORD m_dwID = 0U;

    long m_lOriginX = 0L;
    long m_lOriginY = 0L;
    long m_lSizeX = 0L;
    long m_lSizeY = 0L;

    long m_lLeft = 0L;
    long m_lRight = 0L;
    long m_lTop = 0L;
    long m_lBottom = 0L;

    std::vector<D3DXVECTOR2> m_TempMonsterPosVector;
};

using TMonsterAreaInfoPtrVector = std::vector<CMonsterAreaInfo *>;
using TMonsterAreaInfoPtrVectorIterator = TMonsterAreaInfoPtrVector::iterator;

#endif // !defined(AFX_MONSTERAREAINFO_H__B177E257_2EB2_4CE5_89D3_D06B5618BE1B__INCLUDED_)
