<template>
  <Modal
    :show="show"
    :close="cancel"
    :title="$t('settings.closePrompt.title')"
    width="26rem"
  >
    <p>{{ $t('settings.closePrompt.description') }}</p>
    <label class="remember">
      <input v-model="remember" type="checkbox" />
      <span>{{ $t('settings.closePrompt.remember') }}</span>
    </label>
    <template #footer>
      <button @click="choose('minimizeToTray')">
        {{ $t('settings.closeAppOption.minimizeToTray') }}
      </button>
      <button class="primary" @click="choose('exit')">
        {{ $t('settings.closeAppOption.exit') }}
      </button>
    </template>
  </Modal>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import Modal from './Modal.vue';

export default defineComponent({
  name: 'ModalCloseApp',
  components: { Modal },
  props: {
    show: Boolean,
  },
  emits: {
    cancel: () => true,
    choose: (payload: {
      action: 'exit' | 'minimizeToTray';
      remember: boolean;
    }) => typeof payload.remember === 'boolean',
  },
  data() {
    return { remember: false };
  },
  methods: {
    cancel() {
      this.remember = false;
      this.$emit('cancel');
    },
    choose(action: 'exit' | 'minimizeToTray') {
      this.$emit('choose', { action, remember: this.remember });
      this.remember = false;
    },
  },
});
</script>

<style lang="scss" scoped>
p {
  margin: 0 0 18px;
  line-height: 1.6;
}

.remember {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}
</style>
