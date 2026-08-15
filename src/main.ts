import { createApp } from 'vue'

import ChatApp from './ChatApp.vue'
import ChatOverlayView from './components/ChatOverlayView.vue'
import TranslationHudView from './components/TranslationHudView.vue'
import './styles.css'

const params = new URLSearchParams(window.location.search)
const rootComponent = params.has('translation-hud')
  ? TranslationHudView
  : params.has('chat-overlay') || (import.meta.env.DEV && params.has('overlay-preview'))
    ? ChatOverlayView
    : ChatApp

createApp(rootComponent).mount('#app')
