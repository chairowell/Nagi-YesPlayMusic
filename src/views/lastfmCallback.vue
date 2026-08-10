<template>
  <div class="lastfm-callback">
    <div class="section-1">
      <img src="/img/logos/yesplaymusic.png" />
      <svg-icon icon-class="x"></svg-icon>
      <img src="/img/logos/lastfm.png" />
    </div>
    <div class="message">{{ message }}</div>
    <button v-show="done" @click="close"> 完成 </button>
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import { authGetSession, readLastfmCallbackToken } from '@/api/lastfm';
import { mapActions } from 'pinia';
import { useAppStore } from '@/stores/app';
import {
  closeLastfmAuthorizationWindow,
  persistAuthorizedLastfmSession,
  publishDesktopLastfmAuthorization,
} from '@/services/lastfmAuth';

export default defineComponent({
  name: 'LastfmCallback',
  data() {
    return { message: '请稍等...', done: false };
  },
  async created() {
    const token = readLastfmCallbackToken(window.location);
    if (!token) {
      this.message = '连接失败，请重试或联系开发者（无Token）';
      this.done = true;
      return;
    }
    try {
      const result = await authGetSession(token);
      if (!result.data.session) {
        this.message = '连接失败，请重试或联系开发者（无Session）';
        this.done = true;
        return;
      }
      const session = persistAuthorizedLastfmSession(
        result.data.session,
        localStorage
      );
      this.updateLastfm(session);
      await publishDesktopLastfmAuthorization(session);
      this.message = '已成功连接到 Last.fm';
      this.done = true;
    } catch (error) {
      console.error(
        '[lastfm] failed to exchange the authorization token',
        error
      );
      this.message = '连接失败，请重试或联系开发者';
      this.done = true;
    }
  },
  methods: {
    ...mapActions(useAppStore, ['updateLastfm']),
    async close() {
      if (!(await closeLastfmAuthorizationWindow())) window.close();
    },
  },
});
</script>

<style lang="scss" scoped>
.lastfm-callback {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  height: calc(100vh - 192px);
}
.section-1 {
  margin-bottom: 16px;
  display: flex;
  align-items: center;
  img {
    height: 64px;
    margin: 20px;
  }
  .svg-icon {
    height: 24px;
    width: 24px;
    color: rgba(82, 82, 82, 0.28);
  }
}

.message {
  font-size: 1.4rem;
  font-weight: 500;
  color: var(--color-text);
}

button {
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
  font-weight: 600;
  background-color: var(--color-primary-bg);
  color: var(--color-primary);
  border-radius: 8px;
  margin-top: 24px;
  transition: 0.2s;
  padding: 8px 16px;
  &:hover {
    transform: scale(1.06);
  }
  &:active {
    transform: scale(0.94);
  }
}
</style>
