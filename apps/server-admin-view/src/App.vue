<script setup lang="ts">
import { Toaster } from "@/components/ui/sonner";
import "vue-sonner/style.css";
import { useOnboardingGuide } from "./services/onboarding-guide";

useOnboardingGuide();
</script>

<template>
  <RouterView />
  <Toaster position="top-center" :duration="3000" />
</template>
