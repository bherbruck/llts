type Data = { name: string; value: i64; };

const arr: Array<Data> = [{ name: "One", value: 1 }];

function main(): void {
  arr.push({ name: "Two", value: 2 });
  arr.push({ name: "Three", value: 3 });
  console.log(arr.length);

  for (const item of arr) {
    console.log(item.name);
  }

  const last: Data = arr.pop();
  console.log(last.name);
  console.log(arr.length);
}
