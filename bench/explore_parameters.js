const asciichart = require('asciichart')
const fs = require('fs')
const {spawnSync} = require('child_process')

const gmean = list => (
  Math.pow(list.reduce((a, b) => a*b, 1), 1/list.length)
)

const names = ["automerge-paper", "rustcode", "sveltecomponent", "seph-blog1"]
const getScore = () => {

  const data = names.map(name => {
    const est_file = `../target/criterion/realworld/JumpRope/${name}/new/estimates.json`
    const estimates = JSON.parse(fs.readFileSync(est_file, 'utf8'))

    const bench_file = `../target/criterion/realworld/JumpRope/${name}/new/benchmark.json`
    const elements = JSON.parse(fs.readFileSync(bench_file, 'utf8')).throughput.Elements

    return elements / (estimates.mean.point_estimate / 1e9)
  })
  // console.log(data)
  return data
}

const setSize = size => {
//   fs.writeFileSync('../src/params.rs', `
// pub const XX_SIZE: usize = 380;
// pub const XX_BIAS: u8 = ${size};
// `)
  fs.writeFileSync('../src/params.rs', `
pub const XX_SIZE: usize = ${size};
pub const XX_BIAS: u8 = 65;
`)
}

// const cmd = 'cargo build --release && sleep 3 && taskset 0x1 nice -10 cargo run --release -- --bench --measurement-time=3 -n realworld/JumpRope/automerge-paper'
// const cmd = 'cargo build --release && taskset 0x1 nice -10 cargo run --release -- --bench --measurement-time=10 -n realworld/JumpRope/automerge-paper'
const cmd = 'cargo build --release && taskset 0x1 nice -10 cargo run --release -- --bench --measurement-time=10 -n realworld/JumpRope'
const bench = () => {
  spawnSync(cmd, {
    shell: true,
    stdio: 'inherit',
  })
}

// setSize(100)

const scores = {}
// The first row is the sizes. second row contains mean. Then results.
const scores_arr = new Array(names.length + 2).fill().map(() => [])

const run = size => {
  setSize(size)
  bench()
  const vals = getScore()
  const gm = gmean(vals)
  scores[size] = gm
  scores_arr[0].push(size)
  scores_arr[1].push(gm)
  for (let i = 0; i < vals.length; i++) {
    scores_arr[i+2].push(vals[i])
  }

  console.log(`Registered ${size} => ${gm} (${gm / 1e6})`)
}

// for (let s = 50; s <= 80; s += 5) {
//   run(s)
// }
for (let s = 340; s <= 400; s += 8) {
  run(s)
}
// for (let s = 300; s <= 400; s += 20) {
//   run(s)
// }
console.table(scores)

// run(200)
// console.log(getScore())


const pad = arr => {
  let num = Math.round(80 / (arr.length-1))
  const result = [arr[0]]
  for (let i = 1; i < arr.length; i++) {
    let prev = arr[i-1]
    let next = arr[i]

    for (let j = 1; j <= num; j++) {
      let weight = j/num
      result.push(next * weight + prev * (1-weight))
    }
  }
  return result
}

const drawChart = scores_arr => {
  console.log(asciichart.plot(scores_arr.slice(1).map(pad), {
    colors: [
      asciichart.white,
      asciichart.blue, asciichart.green, asciichart.red, asciichart.yellow
    ],
    height: 50,
  }))
}

// drawChart(JSON.parse(fs.readFileSync('data.json', 'utf8')))

drawChart(scores_arr)

// console.log(asciichart.plot(pad([0, 2, 3]), {
//   colors: [asciichart.blue, asciichart.green, asciichart.red, asciichart.yellow],
//   height: 20,
// }))

fs.writeFileSync('data.json', JSON.stringify(scores_arr))
console.log('data written to data.json')