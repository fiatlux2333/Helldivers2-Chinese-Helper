import { STRATAGEM_PRESETS } from './stratagemPresets'

describe('stratagem presets', () => {
  it('includes the 40-K Meltagun preset from the upstream stratagem database', () => {
    const meltagun = STRATAGEM_PRESETS.find((preset) => preset.id === 'wpn_meltagun')

    expect(meltagun).toEqual({
      id: 'wpn_meltagun',
      group: 'support',
      zhName: '40-K热熔枪',
      enName: '40-KMeltagun',
      sequence: ['KeyS', 'KeyA', 'KeyW', 'KeyA', 'KeyA', 'KeyS'],
    })
  })
})
