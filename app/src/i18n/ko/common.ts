// Shared words (app-language). Korean is the reference catalog: 해요체, one line per item, 리뷰·전역.
export default {
  theme: {
    system: "시스템 설정 따름",
    light: "밝은 테마",
    dark: "어두운 테마",
  },
  language: {
    /** The only choice named in the current language; 한국어 and English are always shown in their own language. */
    system: "시스템 언어 따름",
  },
  actions: {
    close: "닫기",
    cancel: "취소",
    save: "저장",
    /** The fold under a one-line row (MoreInfo). */
    more: "자세히",
  },
  draft: {
    replace: "작성 중인 초안을 바꿀까요?",
  },
} as const;
