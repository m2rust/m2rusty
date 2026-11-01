#pragma once

struct SParticleVertex
{
    D3DXVECTOR3 v3Pos;
    float u = 0.0F, v = 0.0F;
};

struct BlurVertex
{
    D3DXVECTOR3 pos;
    FLOAT rhw;
    DWORD color;
    FLOAT tu, tv;

    static const DWORD FVF;

    BlurVertex(D3DXVECTOR3 p, float w, DWORD c, float u, float v) : pos(p), rhw(w), color(c), tu(u), tv(v)
    {
    }
    ~BlurVertex() = default;
};

class CSnowParticle
{
  public:
    CSnowParticle();
    ~CSnowParticle() = default;

    static CSnowParticle *New();
    static void Delete(CSnowParticle *pSnowParticle);
    static void DestroyPool();

    void Init(const D3DXVECTOR3 &c_rv3Pos);

    void SetCameraVertex(const D3DXVECTOR3 &rv3Up, const D3DXVECTOR3 &rv3Cross);
    bool IsActivate() const;

    void Update(float fElapsedTime, const D3DXVECTOR3 &c_rv3Pos);
    void GetVerticies(SParticleVertex &rv3Vertex1, SParticleVertex &rv3Vertex2, SParticleVertex &rv3Vertex3,
                      SParticleVertex &rv3Vertex4);

  protected:
    bool m_bActivate = false;
    bool m_bChangedSize = false;
    float m_fHalfWidth = 0.0F;
    float m_fHalfHeight = 0.0F;

    D3DXVECTOR3 m_v3Velocity;
    D3DXVECTOR3 m_v3Position;

    D3DXVECTOR3 m_v3Up;
    D3DXVECTOR3 m_v3Cross;

    float m_fPeriod = 0.0F;
    float m_fcurRadian = 0.0F;
    float m_fAmplitude = 0.0F;

  public:
    static std::vector<CSnowParticle *> ms_kVct_SnowParticlePool;
};
