<template>
  <div v-show="show">
    <div class="special-playlist">
      <div class="title gradient"> 每日歌曲推荐 </div>
      <div class="subtitle">根据你的音乐口味生成 · 每天6:00更新</div>
    </div>

    <TrackList
      :tracks="dailyTracks"
      type="playlist"
      dbclick-track-func="dailyTracks"
    />
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import { mapActions, mapState } from 'pinia';
import { useAppStore } from '@/stores/app';
import NProgress from 'nprogress';
import { fetchDailyRecommendTracks } from '@/services/recommendSource';
import locale from '@/locale';

import TrackList from '@/components/TrackList.vue';

export default defineComponent({
  name: 'DailyTracks',
  inject: ['appShell'],
  components: {
    TrackList,
  },
  data() {
    return {
      show: false,
    };
  },
  computed: {
    ...mapState(useAppStore, ['player', 'data', 'dailyTracks']),
  },
  created() {
    if (this.dailyTracks.length === 0) {
      setTimeout(() => {
        if (!this.show) NProgress.start();
      }, 1000);
      this.loadDailyTracks();
    } else {
      this.show = true;
    }
    this.appShell.scrollMainTo(0, 0);
  },
  methods: {
    ...mapActions(useAppStore, ['showToast', 'updateDailyTracks']),
    loadDailyTracks() {
      fetchDailyRecommendTracks()
        .then(tracks => {
          this.updateDailyTracks(tracks);
        })
        .catch((error: unknown) => {
          console.warn('[daily-tracks] load failed', error);
          this.showToast(locale.t('toast.dailyTracksLoadFailed'));
        })
        .finally(() => {
          NProgress.done();
          this.show = true;
        });
    },
  },
});
</script>

<style lang="scss" scoped>
.special-playlist {
  margin-top: 192px;
  margin-bottom: 128px;
  border-radius: 1.25em;
  text-align: center;

  @keyframes letterSpacing4 {
    from {
      letter-spacing: 0px;
    }

    to {
      letter-spacing: 4px;
    }
  }

  @keyframes letterSpacing1 {
    from {
      letter-spacing: 0px;
    }

    to {
      letter-spacing: 1px;
    }
  }

  .title {
    font-size: 84px;
    line-height: 1.05;
    font-weight: 700;
    text-transform: uppercase;

    letter-spacing: 4px;
    animation-duration: 0.8s;
    animation-name: letterSpacing4;
    -webkit-text-fill-color: transparent;
    background-clip: text;
    // background-image: linear-gradient(
    //   225deg,
    //   var(--color-primary),
    //   var(--color-primary)
    // );

    img {
      height: 78px;
      border-radius: 0.125em;
      margin-right: 24px;
    }
  }
  .subtitle {
    font-size: 18px;
    letter-spacing: 1px;
    margin: 28px 0 54px 0;
    animation-duration: 0.8s;
    animation-name: letterSpacing1;
    text-transform: uppercase;
    color: var(--color-text);
  }
  .buttons {
    margin-top: 32px;
    display: flex;
    justify-content: center;
    button {
      margin-right: 16px;
    }
  }
}

.gradient {
  background: linear-gradient(to left, #dd2476, #ff512f);
}
</style>
