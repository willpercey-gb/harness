import { createApp } from 'vue'
import { createPinia } from 'pinia'
import * as Sentry from '@sentry/vue'
import App from './App.vue'
import { router } from './router'
import './scss/app.scss'

const app = createApp(App)

// Sentry: DSN is read from VITE_SENTRY_DSN at build time. Vite inlines
// VITE_* env vars; without it set, this whole block is dead code.
// Public clones build cleanly without any Sentry configuration.
const dsn = import.meta.env.VITE_SENTRY_DSN as string | undefined
if (dsn) {
  Sentry.init({
    app,
    dsn,
    release: (import.meta.env.VITE_APP_VERSION as string) || undefined,
    environment: (import.meta.env.VITE_SENTRY_ENV as string) || 'dev',
    sendDefaultPii: false,
    integrations: [Sentry.browserTracingIntegration({ router })],
    tracesSampleRate: 0.1,
  })
}

app.use(createPinia())
app.use(router)
app.mount('#app')
