<template>
  <span class="artist-in-line">
    {{ computedPrefix }}
    <span v-for="(ar, index) in filteredArtists" :key="index">
      <router-link v-if="ar.id !== 0" :to="`/artist/${ar.id}`">{{
        ar.name
      }}</router-link>
      <span v-else>{{ ar.name }}</span>
      <span v-if="index !== filteredArtists.length - 1" class="separator"
        >,</span
      >
    </span>
  </span>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import type { PropType } from 'vue';
import type { Artist } from '@/types/domain';
export default defineComponent({
  name: 'ArtistInLine',
  props: {
    artists: {
      type: Array as PropType<Artist[]>,
      required: true,
    },
    exclude: {
      type: String,
      default: '',
    },
    prefix: {
      type: String,
      default: '',
    },
  },
  computed: {
    filteredArtists(): Artist[] {
      return this.artists.filter(artist => artist.name !== this.exclude);
    },
    computedPrefix() {
      if (this.filteredArtists.length !== 0) return this.prefix;
      else return '';
    },
  },
});
</script>

<style lang="scss" scoped>
.separator {
  /* make separator distinct enough in long list */
  margin-left: 1px;
  margin-right: 4px;
  position: relative;
  top: 0.5px;
}
</style>
