import react from "@eslint-react/eslint-plugin";
import reactHooks from "eslint-plugin-react-hooks";

const files = ["**/*.jsx", "**/*.tsx"];

export default [
	{ ...react.configs["recommended-typescript"], files },
	{ ...reactHooks.configs.flat.recommended, files },
];

export const strict = [
	{
		files: files,
		rules: {
			"@eslint-react/no-class-component": "error",
			"@eslint-react/no-misused-capture-owner-stack": "error",
			"@eslint-react/no-unstable-context-value": "error",
			"@eslint-react/no-unstable-default-props": "error",
			"@eslint-react/jsx-no-useless-fragment": "error",
			"@eslint-react/dom-no-missing-button-type": "error",
			"@eslint-react/dom-no-missing-iframe-sandbox": "error",
			"@eslint-react/dom-no-unsafe-target-blank": "error",
		},
	},
];
