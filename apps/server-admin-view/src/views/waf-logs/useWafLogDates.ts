import { ref } from "vue";

export const getTodayString = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

export const useWafLogDates = () => {
  const availableDates = ref<string[]>([getTodayString()]);
  const selectedDate = ref(getTodayString());
  const applyDates = (dates: string[], preferred?: string) => {
    const fallbackToday = getTodayString();
    const nextDates = dates.length > 0 ? dates : [fallbackToday];
    availableDates.value = nextDates;
    if (preferred && nextDates.includes(preferred)) {
      selectedDate.value = preferred;
    } else if (!nextDates.includes(selectedDate.value)) {
      selectedDate.value = nextDates.includes(fallbackToday)
        ? fallbackToday
        : nextDates[0] || fallbackToday;
    }
  };

  return { availableDates, selectedDate, applyDates };
};
