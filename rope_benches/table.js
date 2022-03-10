const fs = require('fs')

const datasets = ["automerge-paper", "rustcode", "sveltecomponent", "seph-blog1"]
const algorithms = ['String', 'XiRope', 'Ropey', 'C-JumpRope', 'JumpRope']

console.log('| Dataset | Raw string | XiRope | Ropey | librope (C) | Jumprope |')
console.log('|---------|------------|--------|-------|-------------|----------|')

const roundN = n => Math.round(n * 100) / 100

for (const ds of datasets) {
  const row = `${ds} | ` + algorithms.map(alg => {
    const filename = `../target/criterion/realworld/${alg}/${ds}/new/estimates.json`

    if (fs.existsSync(filename)) {
      const data = JSON.parse(fs.readFileSync(filename, 'utf8')).mean.point_estimate / 1e6
      return `${roundN(data)} ms`
    } else {
      return 'DNF'
    }
  }).join(' | ')

  console.log(row)
}