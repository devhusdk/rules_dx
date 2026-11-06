import vue from "eslint-plugin-vue";

export default [
	...vue.configs["flat/recommended"],
	{
		files: ["**/*.vue"],
		languageOptions: { parserOptions: { parser: "@typescript-eslint/parser" } },
	},
];

export const strict = [
	{
		files: ["**/*.vue"],
		rules: {
			"vue/match-component-import-name": "error",
			"vue/max-props": "error",
			"vue/max-template-depth": "error",
			"vue/no-constant-condition": "error",
			"vue/no-content-outside-block": "error",
			"vue/no-custom-modifiers-on-v-model": "error",
			"vue/no-define-model-default": "error",
			"vue/no-empty-pattern": "error",
			"vue/no-import-compiler-macros": "error",
			"vue/no-irregular-whitespace": "error",
			"vue/no-loss-of-precision": "error",
			"vue/no-multiple-template-root": "error",
			"vue/no-props-shadow": "error",
			"vue/no-ref-object-reactivity-loss": "error",
			"vue/no-shadow-native-events": "error",
			"vue/no-sparse-arrays": "error",
			"vue/no-template-target-blank": "error",
			"vue/no-this-in-before-route-enter": "error",
			"vue/prefer-prop-type-boolean-first": "error",
			"vue/require-default-export": "error",
			"vue/require-explicit-slots": "error",
			"vue/restricted-component-names": "error",
			"vue/v-if-else-key": "error",
		},
	},
];
