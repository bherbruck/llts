// Should error: `eval` is rejected in the compilable subset.

function bad(): void {
  eval("console.log('hello')");
}
