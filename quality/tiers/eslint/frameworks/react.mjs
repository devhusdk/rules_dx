import react from "@eslint-react/eslint-plugin";
import reactHooks from "eslint-plugin-react-hooks";

const files = ["**/*.jsx", "**/*.tsx"];

export default [
	{ ...react.configs["recommended-typescript"], files },
	{ ...reactHooks.configs.flat.recommended, files },
];
