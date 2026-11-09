module.exports.fuzz = function fuzz(data) {
  if (data.length >= 2 && data[0] === 0x46 && data[1] === 0x55) {
    throw new Error("found FU");
  }
};
