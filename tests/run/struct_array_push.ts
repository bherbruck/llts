type Data = { name: string; value: i64; };

const arr: Array<Data> = [{ name: "One", value: 1 }];

function main(): void {
  arr.push({ name: "Two", value: 2 });
  console.log(arr.length);
  console.log(arr[0].name);
  console.log(arr[0].value);
  console.log(arr[1].name);
  console.log(arr[1].value);
}
