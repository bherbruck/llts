// Should error: `any` type is not allowed in the compilable subset.

function bad(x: any): any {
  return x;
}
