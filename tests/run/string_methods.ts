// Expected output: H\n72\n7\ntrue\nHELLO WORLD\nhello world\nWorld\ntrue\ntrue

function main(): void {
  const s: string = "Hello World";
  console.log(s.charAt(0));
  console.log(s.charCodeAt(0));
  console.log(s.indexOf("orld"));
  console.log(s.includes("World"));
  console.log(s.toUpperCase());
  console.log(s.toLowerCase());
  const sliced: string = s.slice(6, 11);
  console.log(sliced);
  console.log(s.startsWith("Hello"));
  console.log(s.endsWith("World"));
}
