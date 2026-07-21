<script setup lang="ts">
import { computed } from 'vue'

import type { TargetDiagnostic } from '@/types/ipc'

const props = defineProps<{
  diagnostic: TargetDiagnostic | null
  loading: boolean
  available: boolean
}>()

const emit = defineEmits<{
  capture: []
}>()

const statusLabel = computed(() => {
  if (props.loading) return '检测中'
  if (!props.diagnostic) return '等待检测'
  if (props.diagnostic.valid) return '目标可用'

  const labels: Record<TargetDiagnostic['status'], string> = {
    ready: '目标可用',
    not_found: '未发现目标',
    title_mismatch: '标题不匹配',
    not_visible: '窗口不可见',
    minimized: '窗口已最小化',
    cloaked: '窗口已隐藏',
    permission_mismatch: '权限不兼容',
    permission_unknown: '权限待确认',
    unsupported_platform: '浏览器预览',
    error: '检测失败',
  }
  return labels[props.diagnostic.status]
})

const statusTone = computed(() => {
  if (props.loading || !props.diagnostic) return 'neutral'
  if (props.diagnostic.valid) return 'ready'
  if (props.diagnostic.status === 'unsupported_platform') return 'preview'
  return 'warning'
})

const processLabel = computed(() => {
  const identity = props.diagnostic?.identity
  return identity ? `PID ${identity.processId} · HWND ${identity.hwnd}` : '尚未锁定窗口'
})
</script>

<template>
  <section
    class="target-panel"
    aria-labelledby="target-heading"
    aria-live="polite"
    :aria-busy="loading"
  >
    <div class="section-heading">
      <div>
        <p class="eyebrow">TARGET PROBE</p>
        <h2 id="target-heading">当前目标</h2>
      </div>
      <button
        class="secondary-button"
        type="button"
        :disabled="loading || !available"
        @click="emit('capture')"
      >
        <svg aria-hidden="true" viewBox="0 0 24 24">
          <path d="M20 7v5h-5M4 17v-5h5" />
          <path d="M6.1 9A7 7 0 0 1 18.7 7L20 12M4 12l1.3 5A7 7 0 0 0 17.9 15" />
        </svg>
        {{ loading ? '捕获中' : available ? '捕获 HD2' : '仅桌面版' }}
      </button>
    </div>

    <div class="target-summary">
      <span class="status-dot" :data-tone="statusTone" aria-hidden="true"></span>
      <div class="target-copy">
        <strong>{{ statusLabel }}</strong>
        <span>{{ diagnostic?.title || processLabel }}</span>
      </div>
      <span class="target-platform">{{ diagnostic?.platform || 'unknown' }}</span>
    </div>

    <p class="target-message">{{ diagnostic?.message || '正在等待第一次目标诊断。' }}</p>

    <dl class="diagnostic-grid">
      <div>
        <dt>标题匹配</dt>
        <dd>{{ diagnostic?.matched ? '是' : '否' }}</dd>
      </div>
      <div>
        <dt>窗口状态</dt>
        <dd>{{ diagnostic?.visible && !diagnostic?.minimized ? '可见' : '不可用' }}</dd>
      </div>
      <div>
        <dt>权限级别</dt>
        <dd>
          {{
            diagnostic?.integrity?.compatible === true
              ? '兼容'
              : diagnostic?.integrity?.compatible === false
                ? '不兼容'
                : '待确认'
          }}
        </dd>
      </div>
    </dl>
  </section>
</template>
