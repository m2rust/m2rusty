#include <Neoeter/Effect/EffectData.h>
#include <Neoeter/Effect/EffectInstance.h>
#include <Neoeter/Effect/EffectManager.h>
#include <Neoeter/Effect/EffectMesh.h>
#include <Neoeter/Effect/EffectMeshInstance.h>
#include <Neoeter/Effect/EmitterProperty.h>
#include <Neoeter/Effect/FrameController.h>
#include <Neoeter/Effect/ParticleInstance.h>
#include <Neoeter/Effect/ParticleProperty.h>
#include <Neoeter/Effect/ParticleSystemData.h>
#include <Neoeter/Effect/ParticleSystemInstance.h>
#include <Neoeter/Effect/SimpleLightData.h>
#include <Neoeter/Effect/SimpleLightInstance.h>
#include <Neoeter/Effect/StdAfx.h>
#include <Neoeter/Effect/Type.h>

#include <Neoeter/Render/Camera.h>
#include <Neoeter/Render/CullingManager.h>
#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Audio/SoundManager.h>

#ifdef NDEBUG
#undef NDEBUG
#endif
#include <cassert>
#include <cmath>
#include <cstdio>
#include <fstream>
#include <vector>

namespace {

void test_type_and_time_events() {
  // 1. GetTimeEventBlendValue empty vector
  TTimeEventTableFloat emptyTable;
  float val = -1.0f;
  GetTimeEventBlendValue(5.0f, emptyTable, &val);
  assert(val == 0.0f);

  // 2. Single value table
  TTimeEventTableFloat singleTable;
  InsertItemTimeEvent(&singleTable, 10.0f, 42.0f);
  assert(singleTable.size() == 1);
  val = 0.0f;
  GetTimeEventBlendValue(0.0f, singleTable, &val);
  assert(val == 42.0f);
  GetTimeEventBlendValue(15.0f, singleTable, &val);
  assert(val == 42.0f);

  // 3. Multi-point table interpolation
  TTimeEventTableFloat table;
  InsertItemTimeEvent(&table, 0.0f, 0.0f);
  InsertItemTimeEvent(&table, 10.0f, 100.0f);
  InsertItemTimeEvent(&table, 20.0f, 200.0f);
  assert(table.size() == 3);

  // Before first key -> clamped to front
  GetTimeEventBlendValue(-5.0f, table, &val);
  assert(std::fabs(val - 0.0f) < 1e-4f);

  // After last key -> clamped to back
  GetTimeEventBlendValue(25.0f, table, &val);
  assert(std::fabs(val - 200.0f) < 1e-4f);

  // Exact key matches
  GetTimeEventBlendValue(0.0f, table, &val);
  assert(std::fabs(val - 0.0f) < 1e-4f);
  GetTimeEventBlendValue(10.0f, table, &val);
  assert(std::fabs(val - 100.0f) < 1e-4f);
  GetTimeEventBlendValue(20.0f, table, &val);
  assert(std::fabs(val - 200.0f) < 1e-4f);

  // Linear interpolation
  GetTimeEventBlendValue(5.0f, table, &val);
  assert(std::fabs(val - 50.0f) < 1e-4f);

  GetTimeEventBlendValue(15.0f, table, &val);
  assert(std::fabs(val - 150.0f) < 1e-4f);

  // 4. Sorted insertion check
  TTimeEventTableFloat unorderedTable;
  InsertItemTimeEvent(&unorderedTable, 30.0f, 300.0f);
  InsertItemTimeEvent(&unorderedTable, 10.0f, 100.0f);
  InsertItemTimeEvent(&unorderedTable, 20.0f, 200.0f);
  assert(unorderedTable.size() == 3);
  assert(unorderedTable[0].m_fTime == 10.0f);
  assert(unorderedTable[1].m_fTime == 20.0f);
  assert(unorderedTable[2].m_fTime == 30.0f);

  // 5. DWORDCOLOR operations
  DWORDCOLOR c1;
  c1.m_dwColor = 0xffffffff;
  DWORDCOLOR c2 = c1 * 0.5f;
  assert((c2.m_dwColor & 0x00ff00ff) != 0);

  DWORDCOLOR c3;
  c3.m_dwColor = 0x00100010;
  c3 += c3;
  assert((c3.m_dwColor & 0x000000ff) == 0x20);

  DWORD dwRaw = c3;
  assert(dwRaw == c3.m_dwColor);

  std::printf("[PASS] test_type_and_time_events\n");
}

void test_frame_controller() {
  CFrameController controller;
  controller.SetMaxFrame(10);
  controller.SetFrameTime(0.1f);
  controller.SetStartFrame(0);
  controller.SetLoopFlag(TRUE);
  controller.SetLoopCount(0); // infinite

  assert(controller.GetCurrentFrame() == 0);
  assert(controller.isActive());

  // Advance 0.05s -> still frame 0
  controller.Update(0.05f);
  assert(controller.GetCurrentFrame() == 0);

  // Advance another 0.06s (total 0.11s) -> frame 1
  controller.Update(0.06f);
  assert(controller.GetCurrentFrame() == 1);

  // Jump forward multiple frames
  controller.SetCurrentFrame(9);
  assert(controller.GetCurrentFrame() == 9);

  // Step across boundary -> wraps around to 0 because loop is enabled
  controller.Update(0.15f);
  assert(controller.GetCurrentFrame() == 0);

  // Non-looping frame controller
  CFrameController nonLoop;
  nonLoop.SetMaxFrame(3);
  nonLoop.SetFrameTime(0.1f);
  nonLoop.SetStartFrame(0);
  nonLoop.SetLoopFlag(FALSE);

  assert(nonLoop.isActive());
  nonLoop.Update(0.11f); // Frame 1
  assert(nonLoop.isActive());
  assert(nonLoop.GetCurrentFrame() == 1);
  nonLoop.Update(0.10f); // Frame 2
  assert(nonLoop.isActive());
  assert(nonLoop.GetCurrentFrame() == 2);
  nonLoop.Update(0.10f); // Frame 3 (>= MaxFrame) -> deactivates
  assert(!nonLoop.isActive());

  std::printf("[PASS] test_frame_controller\n");
}

void test_emitter_and_particle_properties() {
  CEmitterProperty emitter;
  emitter.Clear();

  emitter.m_dwMaxEmissionCount = 50;
  emitter.m_fCycleLength = 2.0f;
  emitter.m_bCycleLoopFlag = TRUE;
  emitter.m_iLoopCount = 5;
  emitter.m_byEmitterShape = CEmitterProperty::EMITTER_SHAPE_ELLIPSE;
  emitter.m_byEmitterAdvancedType =
      CEmitterProperty::EMITTER_ADVANCED_TYPE_OUTER;
  emitter.m_bEmitFromEdgeFlag = TRUE;
  emitter.m_fEmittingRadius = 150.0f;

  assert(emitter.GetMaxEmissionCount() == 50);
  assert(emitter.GetCycleLength() == 2.0f);
  assert(emitter.isCycleLoop() == TRUE);
  assert(emitter.GetLoopCount() == 5);
  assert(emitter.GetEmitterShape() == CEmitterProperty::EMITTER_SHAPE_ELLIPSE);
  assert(emitter.GetEmitterAdvancedType() ==
         CEmitterProperty::EMITTER_ADVANCED_TYPE_OUTER);
  assert(emitter.isEmitFromEdge() == TRUE);

  InsertItemTimeEvent(&emitter.m_TimeEventEmittingVelocity, 0.0f, 50.0f);
  InsertItemTimeEvent(&emitter.m_TimeEventEmittingVelocity, 2.0f, 100.0f);

  float vel = 0.0f;
  emitter.GetEmittingVelocity(1.0f, &vel);
  assert(std::fabs(vel - 75.0f) < 1e-4f);

  // Particle property
  CParticleProperty particle;
  particle.Clear();
  particle.m_byTexAniType = CParticleProperty::TEXTURE_ANIMATION_TYPE_CW;
  particle.m_fTexAniDelay = 0.033f;
  particle.m_byBillboardType = BILLBOARD_TYPE_ALL;
  particle.m_bAttachFlag = FALSE;

  InsertItemTimeEvent(&particle.m_TimeEventScaleX, 0.0f, 1.0f);
  InsertItemTimeEvent(&particle.m_TimeEventScaleX, 1.0f, 2.0f);

  assert(particle.GetTextureAnimationType() ==
         CParticleProperty::TEXTURE_ANIMATION_TYPE_CW);
  assert(std::fabs(particle.GetTextureAnimationFrameDelay() - 0.033f) < 1e-4f);

  CParticleProperty copiedParticle;
  copiedParticle = particle;
  assert(copiedParticle.GetTextureAnimationType() ==
         CParticleProperty::TEXTURE_ANIMATION_TYPE_CW);
  assert(copiedParticle.m_TimeEventScaleX.size() == 2);

  std::printf("[PASS] test_emitter_and_particle_properties\n");
}

class TestLightData : public CLightData {
public:
  void Setup(float duration, float maxRange, const D3DXCOLOR &ambient,
             const D3DXCOLOR &diffuse, BOOL loop, int loopCount) {
    m_fDuration = duration;
    m_fMaxRange = maxRange;
    m_cAmbient = ambient;
    m_cDiffuse = diffuse;
    m_bLoopFlag = loop;
    m_iLoopCount = loopCount;
  }
  void AddRangeKey(float time, float val) {
    InsertItemTimeEvent(&m_TimeEventTableRange, time, val);
  }
};

void test_simple_light() {
  TestLightData lightData;
  lightData.Setup(1.5f, 300.0f, D3DXCOLOR(0.1f, 0.2f, 0.3f, 1.0f),
                  D3DXCOLOR(0.8f, 0.7f, 0.6f, 1.0f), TRUE, 2);
  lightData.AddRangeKey(0.0f, 0.5f);
  lightData.AddRangeKey(1.0f, 1.0f);

  assert(lightData.GetDuration() == 1.5f);
  assert(lightData.isLoop() == TRUE);
  assert(lightData.GetLoopCount() == 2);

  float range = 0.0f;
  lightData.GetRange(0.5f, range);
  assert(std::fabs(range - (0.75f * 300.0f)) < 1.0f);

  D3DLIGHT9 d3dLight;
  lightData.InitializeLight(d3dLight);
  assert(d3dLight.Type == D3DLIGHT_POINT);
  assert(std::fabs(d3dLight.Diffuse.r - 0.8f) < 1e-4f);
  assert(std::fabs(d3dLight.Ambient.g - 0.2f) < 1e-4f);

  // Light Instance pool
  CLightInstance *pLightInst = CLightInstance::New();
  assert(pLightInst != nullptr);
  CLightInstance::Delete(pLightInst);

  std::printf("[PASS] test_simple_light\n");
}

class TestParticleInstance : public CParticleInstance {
public:
  void Setup(CParticleProperty *pProp, CEmitterProperty *pEmit, float lifetime,
             const D3DXVECTOR3 &pos, const D3DXVECTOR3 &vel) {
    m_pParticleProperty = pProp;
    m_pEmitterProperty = pEmit;
    m_fLifeTime = lifetime;
    m_fLastLifeTime = lifetime;
    m_v3Position = pos;
    m_v3Velocity = vel;
  }
  const D3DXVECTOR3 &GetVelocity() const { return m_v3Velocity; }
  const D3DXVECTOR3 &GetPosition() const { return m_v3Position; }
};

void test_particle_instance_and_decorator() {
  CParticleSystemData psData;
  CEmitterProperty *pEmitter = psData.GetEmitterPropertyPointer();
  pEmitter->m_dwMaxEmissionCount = 10;
  pEmitter->m_fCycleLength = 1.0f;
  pEmitter->m_byEmitterShape = CEmitterProperty::EMITTER_SHAPE_POINT;
  pEmitter->m_byEmitterAdvancedType =
      CEmitterProperty::EMITTER_ADVANCED_TYPE_FREE;

  CParticleProperty *pParticleProp = psData.GetParticlePropertyPointer();
  InsertItemTimeEvent(&pParticleProp->m_TimeEventGravity, 0.0f, 9.8f);
  InsertItemTimeEvent(&pParticleProp->m_TimeEventAirResistance, 0.0f, 0.1f);
  InsertItemTimeEvent(&pParticleProp->m_TimeEventScaleX, 0.0f, 1.0f);
  InsertItemTimeEvent(&pParticleProp->m_TimeEventScaleY, 0.0f, 1.0f);

  TestParticleInstance particle;
  particle.Setup(pParticleProp, pEmitter, 1.0f, D3DXVECTOR3(0.0f, 0.0f, 100.0f),
                 D3DXVECTOR3(10.0f, 0.0f, 0.0f));

  psData.BuildDecorator(&particle);

  // Update particle
  BOOL alive = particle.Update(0.1f, 0.0f);
  assert(alive == TRUE);

  // Velocity should be affected by air resistance and gravity
  assert(particle.GetVelocity().z < 0.0f);  // gravity pulled down
  assert(particle.GetVelocity().x < 10.0f); // air resistance slowed x

  // Check position advancement
  assert(particle.GetPosition().x > 0.0f);

  CParticleInstance::DestroySystem();

  std::printf("[PASS] test_particle_instance_and_decorator\n");
}

void test_effect_data_and_manager() {
  const char *scriptPath = "/tmp/test_effect.mse";
  {
    std::ofstream out(scriptPath);
    out << "boundingsphereradius 75.0\n";
    out << "boundingsphereposition 10.0 20.0 30.0\n";
  }

  CEffectData *pData = CEffectData::New();
  assert(pData != nullptr);

  bool loaded = pData->LoadScript(scriptPath);
  assert(loaded);
  assert(std::fabs(pData->GetBoundingSphereRadius() - 75.0f) < 1e-4f);
  D3DXVECTOR3 bPos = pData->GetBoundingSpherePosition();
  assert(std::fabs(bPos.x - 10.0f) < 1e-4f);
  assert(std::fabs(bPos.y - 20.0f) < 1e-4f);
  assert(std::fabs(bPos.z - 30.0f) < 1e-4f);

  CEffectData::Delete(pData);

  // Effect Manager lifecycle
  CEffectManager effectMgr;
  BOOL registered = effectMgr.RegisterEffect(scriptPath, true, false);
  assert(registered);

  int effectIndex = effectMgr.CreateEffect(
      scriptPath, D3DXVECTOR3(0.0f, 0.0f, 0.0f), D3DXVECTOR3(0.0f, 0.0f, 0.0f));
  assert(effectIndex > 0);
  assert(effectMgr.IsAliveEffect(effectIndex));

  effectMgr.SelectEffectInstance(effectIndex);
  effectMgr.SetEffectInstancePosition(D3DXVECTOR3(100.0f, 200.0f, 50.0f));
  effectMgr.SetEffectInstanceRotation(D3DXVECTOR3(0.0f, 45.0f, 0.0f));

  effectMgr.DeactiveEffectInstance(effectIndex);
  bool destroyed = effectMgr.DestroyEffectInstance(effectIndex);
  assert(destroyed);
  assert(!effectMgr.IsAliveEffect(effectIndex));

  effectMgr.DeleteAllInstances();
  effectMgr.Destroy();

  std::remove(scriptPath);
  std::printf("[PASS] test_effect_data_and_manager\n");
}

void test_mesh_script_and_instance() {
  CEffectMeshScript *pScript = CEffectMeshScript::New();
  assert(pScript != nullptr);

  pScript->ReserveMeshData(2);
  assert(pScript->GetMeshDataCount() == 2);
  assert(pScript->CheckMeshIndex(0));
  assert(pScript->CheckMeshIndex(1));
  assert(!pScript->CheckMeshIndex(2));

  CEffectMeshScript::TMeshData *pMeshData = nullptr;
  bool bGotPointer = pScript->GetMeshDataPointer(0, &pMeshData);
  assert(bGotPointer);
  assert(pMeshData != nullptr);

  pMeshData->byBillboardType = MESH_BILLBOARD_TYPE_ALL;
  pMeshData->bBlendingEnable = TRUE;
  pMeshData->byBlendingSrcType = D3DBLEND_SRCALPHA;
  pMeshData->byBlendingDestType = D3DBLEND_INVSRCALPHA;

  assert(pScript->GetBillboardType(0) == MESH_BILLBOARD_TYPE_ALL);
  assert(pScript->isBlendingEnable(0) == TRUE);
  assert(pScript->GetBlendingSrcType(0) == D3DBLEND_SRCALPHA);
  assert(pScript->GetBlendingDestType(0) == D3DBLEND_INVSRCALPHA);

  CEffectMeshScript::Delete(pScript);

  CEffectMeshInstance *pMeshInst = CEffectMeshInstance::New();
  assert(pMeshInst != nullptr);
  CEffectMeshInstance::Delete(pMeshInst);

  std::printf("[PASS] test_mesh_script_and_instance\n");
}

} // namespace

int main() {
  CTimer timer;
  CCullingManager cullingMgr;
  CEterPackManager packMgr;
  CSoundManager soundMgr;

  test_type_and_time_events();
  test_frame_controller();
  test_emitter_and_particle_properties();
  test_simple_light();
  test_particle_instance_and_decorator();
  test_effect_data_and_manager();
  test_mesh_script_and_instance();

  CEffectInstance::DestroySystem();
  CEffectData::DestroySystem();
  CEffectMesh::SEffectMeshData::DestroySystem();
  CEffectMeshScript::DestroySystem();
  CParticleSystemData::DestroySystem();
  CParticleSystemInstance::DestroySystem();
  CLightData::DestroySystem();
  CLightInstance::DestroySystem();

  std::printf("All EffectLib smoke tests passed!\n");
  return 0;
}
